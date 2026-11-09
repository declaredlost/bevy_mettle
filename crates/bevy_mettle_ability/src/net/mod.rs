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
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::AbilityId;

pub use self::end::{AbilityCancelRequest, AbilityEnd};
pub(crate) use self::end::{EarlyCancels, ask_cancel, hear_cancels, hear_ends, tell_end};
pub use self::input::AbilityInput;
pub(crate) use self::input::{add_input, send_input};
pub use self::replica::AbilityReplica;
pub(crate) use self::replica::{show_replicas, sync_replicas};
pub use self::request::{AbilityReply, AbilityRequest, AbilityRequestWith};
pub(crate) use self::request::{
    Pending, answer, predict, predict_granted, predict_tagged, predict_with, reject, resolve, serve,
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

/// The bit an activation derived from another has set: a predicted ability
/// triggered by an event sent under its parent's activation.
pub(crate) const DERIVED: u64 = 1 << 62;

/// The activation of `ability`, triggered under `parent`: the same wherever
/// it's derived, so a client and its server agree on it without asking. A
/// server's parent gives a server's activation.
pub(crate) fn derived(parent: u64, ability: AbilityId) -> u64 {
    // SplitMix64 over the parent and the ability: stable everywhere.
    let mut mixed = parent ^ (ability.index() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    mixed ^= mixed >> 31;
    (mixed & !(SERVERS | DERIVED)) | DERIVED | (parent & SERVERS)
}

#[cfg(test)]
mod tests {
    use bevy_mettle_core::registry::AbilityId;

    use super::{DERIVED, SERVERS, derived};

    #[test]
    fn a_derived_activation_is_the_same_wherever_its_derived() {
        let ability = AbilityId::new(3);
        assert_eq!(derived(7, ability), derived(7, ability));
    }

    #[test]
    fn a_derived_activation_differs_by_parent_and_by_ability() {
        let ability = AbilityId::new(3);
        assert_ne!(derived(7, ability), derived(8, ability), "by parent");
        assert_ne!(
            derived(7, ability),
            derived(7, AbilityId::new(4)),
            "by ability"
        );
    }

    #[test]
    fn a_derived_activation_has_the_derived_bit_and_keeps_its_parents_servers_bit() {
        let ability = AbilityId::new(3);
        for parent in [0, 1, 7, u64::from(u32::MAX), DERIVED | 5] {
            let clients = derived(parent, ability);
            assert_ne!(clients & DERIVED, 0, "derived, from {parent}");
            assert_eq!(clients & SERVERS, 0, "a client's, from {parent}");
            let servers = derived(parent | SERVERS, ability);
            assert_ne!(servers & DERIVED, 0, "derived, from the server's {parent}");
            assert_ne!(servers & SERVERS, 0, "the server's, from its {parent}");
        }
    }

    #[test]
    fn a_derived_activation_derived_again_is_another() {
        let ability = AbilityId::new(3);
        let child = derived(7, ability);
        assert_ne!(derived(child, ability), child);
    }
}
