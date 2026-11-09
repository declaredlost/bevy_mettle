//! Asking the server to activate an ability, and its answer: predicted on the
//! client, served on the server, resolved as it comes back.

use std::any::TypeId;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_effect::EffectWorld;
use serde::{Deserialize, Serialize};

use super::{SERVERS, role, running};
use crate::active::ActiveAbility;
use crate::behaviour::{Behaviour, BehaviourId, BehaviourRegistry, NoInput};
use crate::events::{AbilityFailed, Failure};
use crate::{AbilityId, engine};

/// A client's ask that the server activate an ability it already predicts.
///
/// It asks for `ability` on `actor`, which the client activated under
/// `activation`.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityRequest {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
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
    pub ability: AbilityId,
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

/// The server's answer to an [`AbilityRequest`]: activated, or why not.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityReply {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
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

/// The abilities a client predicted, by activation, not yet answered.
#[derive(Resource, Debug, Default)]
pub(crate) struct Pending(HashMap<u64, Entity>);

/// Activates `id` for `actor`: on a client, predicted, and asked of the server.
pub(crate) fn predict(world: &mut World, actor: Entity, id: AbilityId) -> Result<Entity, Failure> {
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
        ability: id,
        activation,
    });
    Ok(ability)
}

/// Activates `id` for `actor` with `input`: on a client, predicted, and asked
/// of the server with the input.
pub(crate) fn predict_with<B: Behaviour>(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    input: B::Input,
) -> Result<Entity, Failure> {
    let Some(behaviour) = world
        .get_resource::<BehaviourRegistry>()
        .and_then(|behaviours| behaviours.id_of::<B>())
    else {
        warn!("`{}` isn't added: nothing it runs can activate", B::NAME);
        return Err(Failure::Input);
    };
    if TypeId::of::<B::Input>() == TypeId::of::<NoInput>() {
        // Nothing to give: as any activation, but still of `B`.
        return predict_runs(world, actor, id, behaviour);
    }
    if role(world) != NetRole::Client {
        return engine::activate_id_with(world, actor, id, None, Some(behaviour), Some(input));
    }
    let activation = engine::next_activation(world);
    let ability = engine::activate_id_with(
        world,
        actor,
        id,
        Some(activation),
        Some(behaviour),
        Some(input.clone()),
    )?;
    world
        .get_resource_or_init::<Pending>()
        .0
        .insert(activation, ability);
    world.write_message(AbilityRequestWith {
        actor,
        ability: id,
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
    id: AbilityId,
    behaviour: BehaviourId,
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
        ability: id,
        activation,
    });
    Ok(ability)
}

/// On a server: rejects a client's `activation` back to it, as it's no longer
/// to be (its input didn't hold). A server's own isn't answered.
pub(crate) fn reject(
    world: &mut World,
    actor: Entity,
    ability: AbilityId,
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
                    request.ability,
                    Some(request.activation),
                )
                .err()
            } else {
                warn!(
                    "a client asked to activate {:?} under the server's activation {}: rejected",
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
                engine::end_quietly(world, ability, true);
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
