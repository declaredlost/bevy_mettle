//! Endings over a network: what ran under a client's activation, and cancels.
//!
//! The server telling a client of the end of what ran under its activation,
//! and a client asking the server to cancel its own.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use serde::{Deserialize, Serialize};

use super::{SERVERS, role, running};
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
    pub activation: u64,
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
    pub activation: u64,
    /// Whether it was cancelled, rather than ended.
    pub cancelled: bool,
}

impl MapEntities for AbilityEnd {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.actor = mapper.get_mapped(self.actor);
    }
}

/// On a server: tells the client whose activation `actor`'s ability ran
/// under that it ended here. A server's own activations, and a client's
/// endings, aren't told.
pub(crate) fn tell_end(world: &mut World, actor: Entity, activation: u64, cancelled: bool) {
    if activation & SERVERS == 0 && role(world) == NetRole::Server {
        world.write_message(AbilityEnd {
            actor,
            activation,
            cancelled,
        });
    }
}

/// On a client: asks the server to cancel what `actor` ran under the client's
/// own `activation`, as the client cancelled it.
pub(crate) fn ask_cancel(world: &mut World, actor: Entity, activation: u64) {
    if activation & SERVERS == 0 && role(world) == NetRole::Client {
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
/// activations. One not running yet is held a while, as its request may be
/// yet to come; one still not running then is past.
pub(crate) fn hear_cancels(
    mut requests: MessageReader<AbilityCancelRequest>,
    held: Res<EarlyCancels>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let heard: Vec<AbilityCancelRequest> = requests
        .read()
        .filter(|request| {
            let theirs = request.activation & SERVERS == 0;
            if !theirs {
                warn!(
                    "a client asked to cancel the server's activation {}: ignored",
                    request.activation
                );
            }
            theirs
        })
        .cloned()
        .collect();
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
            if let Some(ability) = running(world, request.actor, request.activation) {
                engine::end_quietly(world, ability, true);
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
