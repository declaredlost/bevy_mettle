//! Endings over a network: what ran under a client's activation, and cancels.
//!
//! The server telling a client of the end of what ran under its activation,
//! and a client asking the server to cancel its own.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::net::NetRole;
use serde::{Deserialize, Serialize};

use super::{role, running};
use crate::active::GrantedAbility;
use crate::data::NetExecution;
use crate::engine;

/// A client's ask that the server cancel an ability it cancelled itself.
///
/// One it ran under its own activation, and cancelled itself: a channel let go
/// of, a cast called off.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityCancelRequest {
    /// The actor.
    pub actor: Entity,
    /// The activation it ran under.
    pub activation: Activation,
}

impl MapEntities for AbilityCancelRequest {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// The server's telling a client that an ability it activated ended there.
///
/// One running under the client's activation, ended on the server:
/// interrupted, cancelled, or done.
#[derive(Message, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityEnd {
    /// The actor.
    pub actor: Entity,
    /// The activation it ran under.
    pub activation: Activation,
    /// Whether it was cancelled, rather than ended.
    pub cancelled: bool,
}

impl MapEntities for AbilityEnd {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// Whether the client controlling an actor runs an ability run as `net`, under
/// `activation`, too: its own predicted activations, and every server-initiated
/// one.
pub(crate) fn client_runs(net: NetExecution, activation: Activation) -> bool {
    match net {
        NetExecution::Predicted => !activation.is_servers(),
        NetExecution::ServerInitiated => true,
        NetExecution::ServerOnly | NetExecution::LocalOnly => false,
    }
}

/// On a server: tells the client running `actor`'s ability too that it ended
/// here: one it predicted, or one the server initiated. What a client doesn't
/// run, and a client's endings, aren't told.
pub(crate) fn tell_end(
    world: &mut World,
    actor: Entity,
    net: NetExecution,
    activation: Activation,
    cancelled: bool,
) {
    if client_runs(net, activation) && role(world) == NetRole::Server {
        world.write_message(AbilityEnd {
            actor,
            activation,
            cancelled,
        });
    }
}

/// On a client: asks the server to cancel what `actor` ran under
/// `activation`, as the client cancelled it: one it predicted, or one the
/// server initiated. A local-only one is the client's alone.
pub(crate) fn ask_cancel(
    world: &mut World,
    actor: Entity,
    net: NetExecution,
    activation: Activation,
) {
    if client_runs(net, activation) && role(world) == NetRole::Client {
        world.write_message(AbilityCancelRequest { actor, activation });
    }
}

/// How long a server holds a cancel of an activation it hasn't seen yet, in
/// seconds: the request it cancels may come after it.
const HOLD: f32 = 1.0;

/// How many cancels a server holds for one actor at most: past it, the
/// oldest is let go.
const HELD_PER_ACTOR: usize = 16;

/// On a server: cancels of activations not running yet, and when each was
/// heard.
#[derive(Resource, Default)]
pub(crate) struct EarlyCancels(Vec<(AbilityCancelRequest, f32)>);

/// On a server: cancels what clients cancelled themselves, under their own
/// activations, or the server's where it initiated a client runs too. One of
/// a client's not running yet is held a while, as its request may be yet to
/// come; one still not running then is past.
pub(crate) fn hear_cancels(
    mut requests: MessageReader<AbilityCancelRequest>,
    held: Res<EarlyCancels>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let heard: Vec<AbilityCancelRequest> = requests.read().cloned().collect();
    if heard.is_empty() && held.0.is_empty() {
        return;
    }
    let now = time.elapsed_secs();
    commands.queue(move |world: &mut World| {
        let mut cancels = std::mem::take(&mut world.resource_mut::<EarlyCancels>().0);
        for request in heard {
            let actor = request.actor;
            if cancels
                .iter()
                .filter(|(held, _)| held.actor == actor)
                .count()
                >= HELD_PER_ACTOR
                && let Some(oldest) = cancels.iter().position(|(held, _)| held.actor == actor)
            {
                let (dropped, _) = cancels.remove(oldest);
                warn!(
                    "too many cancels held for {actor}: activation {} let go",
                    dropped.activation
                );
            }
            cancels.push((request, now));
        }
        cancels.retain(|(request, heard)| {
            let running = running(world, request.actor, request.activation);
            if request.activation.is_servers() {
                // The server's own: running already, if it's the client's
                // to cancel at all.
                let initiated = running.is_some_and(|ability| {
                    world
                        .get::<GrantedAbility>(ability)
                        .is_some_and(|granted| granted.def.net == NetExecution::ServerInitiated)
                });
                if let Some(ability) = running.filter(|_| initiated) {
                    engine::end_quietly(world, ability, true);
                } else {
                    warn!(
                        "a client asked to cancel the server's activation {}, which isn't one \
                         it runs: ignored",
                        request.activation
                    );
                }
                return false;
            }
            if let Some(ability) = running {
                // A client cancels only what it runs: not a server-run one it
                // only asked for.
                let theirs = world
                    .get::<GrantedAbility>(ability)
                    .is_some_and(|granted| client_runs(granted.def.net, request.activation));
                if theirs {
                    engine::end_quietly(world, ability, true);
                } else {
                    warn!(
                        "a client asked to cancel activation {}, which it doesn't run: ignored",
                        request.activation
                    );
                }
                return false;
            }
            now - heard < HOLD
        });
        world.resource_mut::<EarlyCancels>().0 = cancels;
    });
}

/// On a client: ends what the server ended of its activations; one no longer
/// running here ended here already.
pub(crate) fn hear_ends(mut ends: MessageReader<AbilityEnd>, mut commands: Commands) {
    for told in ends.read().cloned() {
        commands.queue(move |world: &mut World| {
            if let Some(ability) = running(world, told.actor, told.activation) {
                engine::end_quietly(world, ability, told.cancelled);
            }
        });
    }
}
