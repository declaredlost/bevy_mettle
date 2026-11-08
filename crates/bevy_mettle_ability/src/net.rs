//! Abilities over a network: a client predicts its own, the server decides.
//!
//! Transport-agnostic: the game (or an adapter) ships [`AbilityRequest`]s to
//! the server, [`AbilityReply`]s back, and [`AbilityReplica`]s with their
//! [`AbilityOf`](crate::active::AbilityOf).

use std::any::TypeId;
use std::collections::HashMap;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_effect::EffectWorld;
use serde::{Deserialize, Serialize};

use crate::active::{ActiveAbility, GrantedAbility};
use crate::behaviour::{AnyBehaviour, Behaviour, NoInput};
use crate::data::Ability;
use crate::events::{AbilityFailed, Failure};
use crate::{AbilityRegistry, engine};

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

/// A client's ask that the server activate an ability with its input, which
/// it already predicts: an [`AbilityRequest`], and the input it activated it
/// with.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityRequestWith<I> {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: Id<Ability>,
    /// The activation the client predicted it under.
    pub activation: u64,
    /// The input it activated it with.
    pub input: I,
}

impl<I: MapEntities> MapEntities for AbilityRequestWith<I> {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
        self.input.map_entities(mapper);
    }
}

/// A client's new input to an ability it's running, which it already
/// predicts: for the server to check and take.
#[derive(Message, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityInput<I> {
    /// The actor.
    pub actor: Entity,
    /// The activation it's running under.
    pub activation: u64,
    /// The input.
    pub input: I,
}

impl<I: MapEntities> MapEntities for AbilityInput<I> {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
        self.input.map_entities(mapper);
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

/// Activates `id` for `actor` with `input`: on a client, predicted, and asked
/// of the server with the input.
pub(crate) fn predict_with<B: Behaviour>(
    world: &mut World,
    actor: Entity,
    id: &Id<Ability>,
    input: B::Input,
) -> Result<Entity, Failure> {
    if TypeId::of::<B::Input>() == TypeId::of::<NoInput>() {
        // Nothing to give: as any activation, but still of `B`.
        return predict_runs(world, actor, id, &B::ID);
    }
    if role(world) != NetRole::Client {
        return engine::activate_id_with(world, actor, id, None, Some(&B::ID), Some(input));
    }
    let activation = engine::next_activation(world);
    let ability = engine::activate_id_with(
        world,
        actor,
        id,
        Some(activation),
        Some(&B::ID),
        Some(input.clone()),
    )?;
    world
        .get_resource_or_init::<Pending>()
        .0
        .insert(activation, ability);
    world.write_message(AbilityRequestWith {
        actor,
        ability: id.clone(),
        activation,
        input,
    });
    Ok(ability)
}

/// Activates `id`, run by `behaviour`, for `actor`, with no input: as
/// [`predict`] does.
fn predict_runs(
    world: &mut World,
    actor: Entity,
    id: &Id<Ability>,
    behaviour: &Id<AnyBehaviour>,
) -> Result<Entity, Failure> {
    if role(world) != NetRole::Client {
        return engine::activate_id_with::<NoInput>(world, actor, id, None, Some(behaviour), None);
    }
    let activation = engine::next_activation(world);
    let ability = engine::activate_id_with::<NoInput>(
        world,
        actor,
        id,
        Some(activation),
        Some(behaviour),
        None,
    )?;
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

/// On a client: sends the server new input to the activation it's running.
pub(crate) fn send_input<I: Component + Clone>(
    world: &mut World,
    actor: Entity,
    activation: u64,
    input: I,
) {
    world.write_message(AbilityInput {
        actor,
        activation,
        input,
    });
}

/// On a server: rejects a client's `activation` back to it, as it's no longer
/// to be (its input didn't hold). A server's own isn't answered.
pub(crate) fn reject(
    world: &mut World,
    actor: Entity,
    ability: Id<Ability>,
    activation: u64,
    reason: Failure,
) {
    if role(world) == NetRole::Server && activation & SERVERS == 0 {
        world.write_message(AbilityReply {
            actor,
            ability,
            activation,
            rejected: Some(reason),
        });
    }
}

/// Adds what carries `I`, a behaviour's input, between a client and its
/// server: once, however many behaviours take it.
pub(crate) fn add_input<I: Component + Clone>(app: &mut App) {
    if app.world().contains_resource::<Messages<AbilityInput<I>>>() {
        return;
    }
    app.init_resource::<NetRole>()
        .add_message::<AbilityRequestWith<I>>()
        .add_message::<AbilityInput<I>>()
        .add_systems(
            PreUpdate,
            (serve_with::<I>, hear_input::<I>)
                .chain()
                .after(serve)
                .in_set(crate::AbilitySystems)
                .run_if(resource_equals(NetRole::Server)),
        );
}

/// On a server: activates what clients ask for with their input, under
/// their activations, and answers.
fn serve_with<I: Component + Clone>(
    mut requests: MessageReader<AbilityRequestWith<I>>,
    mut commands: Commands,
) {
    for request in requests.read().cloned() {
        commands.queue(move |world: &mut World| {
            let rejected = if request.activation & SERVERS == 0 {
                engine::activate_id_with(
                    world,
                    request.actor,
                    &request.ability,
                    Some(request.activation),
                    None,
                    Some(request.input),
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

/// On a server: gives clients' new input to the activations they're running,
/// checked; one no longer running is past, and dropped.
fn hear_input<I: Component + Clone>(
    mut inputs: MessageReader<AbilityInput<I>>,
    mut commands: Commands,
) {
    for given in inputs.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Some(ability) = running(world, given.actor, given.activation) {
                let _ = engine::set_input(world, ability, None, given.input);
            }
        });
    }
}

/// `actor`'s ability running under `activation`, if there is one.
fn running(world: &World, actor: Entity, activation: u64) -> Option<Entity> {
    engine::abilities_of(world, actor)
        .into_iter()
        .find(|ability| {
            world
                .get::<ActiveAbility>(*ability)
                .is_some_and(|active| active.activation == activation)
        })
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
            // Not pending, it was answered before: a running activation whose
            // new input was rejected.
            let ability = world
                .get_resource_mut::<Pending>()
                .and_then(|mut predicted| predicted.0.remove(&reply.activation))
                .or_else(|| running(world, reply.actor, reply.activation));
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
