//! What the server starts for a client to run too: server-initiated abilities.
//!
//! A server-initiated ability started on the server, for the owning client to
//! run its behaviour too.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::AbilityId;
use serde::{Deserialize, Serialize};

use super::role;
use crate::behaviour::NoInput;
use crate::data::{AbilityDef, NetExecution};
use crate::engine;

/// The server's telling the owning client a server-initiated ability started.
///
/// For the client to run its behaviour too, under the server's activation:
/// not checked or paid again. Sent for
/// [`ServerInitiated`](NetExecution::ServerInitiated) abilities, however they
/// started: a client's ask, a trigger, the server's own code. With the input it
/// started with: [`NoInput`] for a behaviour that takes none.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityStarted<I = NoInput> {
    /// The actor.
    pub actor: Entity,
    /// The ability.
    pub ability: AbilityId,
    /// The activation it started under on the server.
    pub activation: Activation,
    /// The input it started with.
    pub input: I,
}

impl<I: MapEntities> MapEntities for AbilityStarted<I> {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
        self.input.map_entities(mapper);
    }
}

/// Whether `world` tells a client `def` started: a server, of a
/// server-initiated ability.
pub(crate) fn tells_start(world: &World, def: &AbilityDef) -> bool {
    role(world) == NetRole::Server && def.net == NetExecution::ServerInitiated
}

/// On a server: tells `actor`'s client the server-initiated ability `def`
/// started under `activation`, with `input` if it took one.
pub(crate) fn started<I: Component + Clone>(
    world: &mut World,
    actor: Entity,
    def: &AbilityDef,
    activation: Activation,
    input: Option<I>,
) {
    let ability = def.id;
    match input {
        Some(input) if world.contains_resource::<Messages<AbilityStarted<I>>>() => {
            world.write_message(AbilityStarted {
                actor,
                ability,
                activation,
                input,
            });
        }
        _ => {
            world.write_message(AbilityStarted {
                actor,
                ability,
                activation,
                input: NoInput,
            });
        }
    }
}

/// On a client: runs what the server started with input `I`, as it says.
pub(crate) fn hear_started<I: Component + Clone>(
    mut started: MessageReader<AbilityStarted<I>>,
    mut commands: Commands,
) {
    for started in started.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Some(ability) = granted(world, started.actor, started.ability) {
                engine::mirror(
                    world,
                    ability,
                    started.activation,
                    super::given(started.input),
                );
            }
        });
    }
}

/// `actor`'s granted ability `id`, if it has it: warned if not.
fn granted(world: &World, actor: Entity, id: AbilityId) -> Option<Entity> {
    let ability = super::request::granted(world, actor, id).map(|(ability, _)| ability);
    if ability.is_none() {
        warn!("the server started {id:?} for {actor}, which hasn't it here");
    }
    ability
}
