//! Abilities over a network: each run as its policy says, the server deciding.
//!
//! Each ability's [`NetExecution`](crate::data::NetExecution) says who runs it:
//! predicted by the client, run by the server alone, initiated by the server
//! and mirrored on the controlling client, or local only.
//!
//! Transport-agnostic: the game (or an adapter) ships [`AbilityRequest`]s to
//! the server, [`AbilityReply`]s back, [`AbilityStarted`]s as the server starts
//! what a client mirrors, [`AbilityEnd`]s as the server ends what a client
//! runs and [`AbilityCancelRequest`]s as a client cancels it, and
//! [`AbilityReplica`]s with their [`AbilityOf`](crate::active::AbilityOf).
//! An ability given input goes as [`AbilityRequestWith`] and
//! [`AbilityStartedWith`], and new input while it runs as [`AbilityInput`].

mod end;
mod input;
mod replica;
mod request;
mod start;

use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::net::NetRole;

pub use self::end::{AbilityCancelRequest, AbilityEnd};
pub(crate) use self::end::{EarlyCancels, ask_cancel, hear_cancels, hear_ends, tell_end};
pub use self::input::AbilityInput;
pub(crate) use self::input::{add_input, send_input};
pub use self::replica::AbilityReplica;
pub(crate) use self::replica::{show_replicas, sync_replicas};
pub use self::request::{
    AbilityReply, AbilityRequest, AbilityRequestWith, PredictionTimeout, abandon_pending,
};
pub(crate) use self::request::{
    Asked, Pending, answer, expire, predict, predict_granted, predict_tagged, predict_with, reject,
    resolve, serve,
};
pub use self::start::{AbilityStarted, AbilityStartedWith};
pub(crate) use self::start::{hear_started, hear_started_with, started, tells_start};
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
fn running(world: &World, actor: Entity, activation: Activation) -> Option<Entity> {
    engine::abilities_of(world, actor)
        .into_iter()
        .find(|ability| {
            world
                .get::<ActiveAbility>(*ability)
                .is_some_and(|active| active.activation == activation)
        })
}
