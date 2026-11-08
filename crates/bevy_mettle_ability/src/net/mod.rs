//! Abilities over a network: a client predicts its own, the server decides.
//!
//! Transport-agnostic: the game (or an adapter) ships [`AbilityRequest`]s to
//! the server, [`AbilityReply`]s back, [`AbilityEnd`]s as the server ends a
//! client's activation and [`AbilityCancelRequest`]s as a client cancels its
//! own, and
//! [`AbilityReplica`]s with their [`AbilityOf`](crate::active::AbilityOf).
//!
//! [`AbilityRequest`]: request::AbilityRequest
//! [`AbilityReply`]: request::AbilityReply
//! [`AbilityEnd`]: end::AbilityEnd
//! [`AbilityCancelRequest`]: end::AbilityCancelRequest
//! [`AbilityReplica`]: replica::AbilityReplica

pub mod end;
pub mod input;
pub mod replica;
pub mod request;

use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;

pub(crate) use self::end::{EarlyCancels, ask_cancel, hear_cancels, hear_ends, tell_end};
pub(crate) use self::input::{add_input, send_input};
pub(crate) use self::replica::{show_replicas, sync_replicas};
pub(crate) use self::request::{Pending, predict, predict_with, reject, resolve, serve};
use crate::active::ActiveAbility;
use crate::engine;

/// On a server: the systems activating what clients ask for, with input and
/// without. Their cancels are heard after them, so a cancel arriving with
/// the request it cancels finds it running.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Serving;

/// This world's role.
pub(crate) fn role(world: &World) -> NetRole {
    world.get_resource::<NetRole>().copied().unwrap_or_default()
}

/// Whether this world leaves what triggers abilities to the server.
pub(crate) fn defers(world: &World) -> bool {
    role(world) == NetRole::Client
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
