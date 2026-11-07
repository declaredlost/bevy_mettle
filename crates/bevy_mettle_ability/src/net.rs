//! Abilities over a network: a client predicts its own, the server decides.
//!
//! Transport-agnostic: the game (or an adapter) ships [`AbilityRequest`]s to
//! the server, [`AbilityReply`]s back, and [`AbilityReplica`]s with their
//! [`AbilityOf`](crate::active::AbilityOf).

use std::collections::HashMap;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_effect::EffectWorld;
use bevy_mettle_effect::net::NetRole;
use serde::{Deserialize, Serialize};

use crate::active::{ActiveAbility, GrantedAbility};
use crate::events::{AbilityFailed, Failure};
use crate::{Ability, AbilityRegistry, engine};

/// A client's ask that the server activate an ability it already predicts.
///
/// It asks for `ability` on `actor`, which the client activated under
/// `activation`.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityRequest {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: Id<Ability>,
    /// The activation the client predicted it under: its own, the top bit
    /// clear (the server's own have it set).
    pub activation: u64,
}

impl MapEntities for AbilityRequest {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// The server's answer to an [`AbilityRequest`]: activated, or why not.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityReply {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: Id<Ability>,
    /// The activation it answers.
    pub activation: u64,
    /// Why it didn't activate, if it didn't.
    pub rejected: Option<Failure>,
}

impl MapEntities for AbilityReply {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// On a server's granted ability: what a client needs to have it too.
///
/// For the game to replicate with its `AbilityOf`.
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityReplica {
    /// The ability.
    pub id: Id<Ability>,
    /// Its level.
    pub level: f32,
}

/// The abilities a client predicted, by activation, not yet answered.
#[derive(Resource, Debug, Default)]
pub(crate) struct Pending(HashMap<u64, Entity>);

/// This world's role.
pub(crate) fn role(world: &World) -> NetRole {
    world.get_resource::<NetRole>().copied().unwrap_or_default()
}

/// Whether this world leaves what triggers abilities to the server.
pub(crate) fn defers(world: &World) -> bool {
    role(world) == NetRole::Client
}

/// Activates `id` for `actor`: on a client, predicted, and asked of the server.
pub(crate) fn predict(
    world: &mut World,
    actor: Entity,
    id: &Id<Ability>,
) -> Result<Entity, Failure> {
    if role(world) != NetRole::Client {
        return engine::activate_id(world, actor, id, None);
    }
    let activation = engine::next_activation(world);
    let ability = engine::activate_id(world, actor, id, Some(activation))?;
    world
        .get_resource_or_init::<Pending>()
        .0
        .insert(activation, ability);
    world.write_message(AbilityRequest {
        actor,
        ability: id.clone(),
        activation,
    });
    Ok(ability)
}

/// The bit a server's own activations have set, and a client's can't.
pub(crate) const SERVERS: u64 = 1 << 63;

/// On a server: activates what clients ask for, under their activations, and
/// answers. One under an activation with the server's bit set is rejected: a
/// client's can't look like the server's own.
pub(crate) fn serve(mut requests: MessageReader<AbilityRequest>, mut commands: Commands) {
    for request in requests.read().cloned() {
        commands.queue(move |world: &mut World| {
            let rejected = if request.activation & SERVERS == 0 {
                engine::activate_id(
                    world,
                    request.actor,
                    &request.ability,
                    Some(request.activation),
                )
                .err()
            } else {
                warn!(
                    "a client asked to activate `{}` under the server's activation {}: rejected",
                    request.ability, request.activation
                );
                Some(Failure::NotGranted)
            };
            world.write_message(AbilityReply {
                actor: request.actor,
                ability: request.ability,
                activation: request.activation,
                rejected,
            });
        });
    }
}

/// On a client: keeps what the server confirmed, and undoes what it rejected.
pub(crate) fn resolve(mut replies: MessageReader<AbilityReply>, mut commands: Commands) {
    for reply in replies.read().cloned() {
        commands.queue(move |world: &mut World| {
            let ability = world
                .get_resource_mut::<Pending>()
                .and_then(|mut predicted| predicted.0.remove(&reply.activation));
            let Some(reason) = reply.rejected else {
                world.confirm_prediction(reply.activation);
                return;
            };
            if let Some(ability) = ability
                && world
                    .get::<ActiveAbility>(ability)
                    .is_some_and(|active| active.activation == reply.activation)
            {
                engine::end(world, ability, true);
            }
            world.reject_prediction(reply.activation);
            world.trigger(AbilityFailed {
                entity: reply.actor,
                id: reply.ability,
                reason,
            });
        });
    }
}

/// On a server: keeps each granted ability's replica.
pub(crate) fn sync_replicas(
    mut commands: Commands,
    granted: Query<(Entity, &GrantedAbility, Option<&AbilityReplica>), Changed<GrantedAbility>>,
) {
    for (entity, granted, replica) in &granted {
        let now = AbilityReplica {
            id: granted.def.id.clone(),
            level: granted.level,
        };
        if replica != Some(&now) {
            commands.entity(entity).insert(now);
        }
    }
}

/// On a client: has each ability the server's replica arrived for, and keeps
/// it as its replica says.
pub(crate) fn show_replicas(
    mut commands: Commands,
    registry: Res<AbilityRegistry>,
    arrived: Query<(Entity, &AbilityReplica), Without<GrantedAbility>>,
    mut changed: Query<(&AbilityReplica, &mut GrantedAbility), Changed<AbilityReplica>>,
) {
    for (entity, replica) in &arrived {
        let Some(def) = registry.get(&replica.id).cloned() else {
            warn!("there's no ability called `{}` to have", replica.id);
            continue;
        };
        commands
            .entity(entity)
            .insert(GrantedAbility::new(def, replica.level, None));
    }
    for (replica, mut granted) in &mut changed {
        if granted.def.id != replica.id {
            let Some(def) = registry.get(&replica.id).cloned() else {
                warn!("there's no ability called `{}` to have", replica.id);
                continue;
            };
            granted.def = def;
        }
        if granted.level.to_bits() != replica.level.to_bits() {
            granted.level = replica.level;
        }
    }
}
