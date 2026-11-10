//! Activations: what an ability's run is known by, here and over a network.

use std::fmt;

use bevy_reflect::prelude::*;
use serde::{Deserialize, Serialize};

use crate::registry::AbilityId;

/// The bit a server's own activations have set, and a client's can't.
const SERVERS: u64 = 1 << 63;

/// The bit an activation derived from another has set: a predicted ability
/// triggered by an event sent under its parent's activation.
const DERIVED: u64 = 1 << 62;

/// The bits counting activations, below the flags.
const COUNTER: u64 = !(SERVERS | DERIVED);

/// An activation of an ability, what its run's work is known by.
///
/// What's done under it (an effect applied, a cue played) is taken back with
/// it if a client's prediction of it is rejected. A client's own and a
/// server's own are apart, so a client predicts under its own without asking.
///
/// Written as its number.
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Serialize,
    Deserialize,
    Reflect
)]
#[reflect(opaque, Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Activation(u64);

impl Activation {
    /// A client's own `n`th activation, or a standalone world's.
    ///
    /// `n` is kept to the counting bits, so it can't set a flag.
    #[must_use]
    pub const fn client(n: u64) -> Self {
        Self(n & COUNTER)
    }

    /// A server's own `n`th activation, apart from any client's.
    #[must_use]
    pub const fn server(n: u64) -> Self {
        Self((n & COUNTER) | SERVERS)
    }

    /// Whether it's the server's own, or derived from the server's own.
    #[must_use]
    pub const fn is_servers(self) -> bool {
        self.0 & SERVERS != 0
    }

    /// Whether it's derived from another, by [`derived`](Self::derived).
    #[must_use]
    pub const fn is_derived(self) -> bool {
        self.0 & DERIVED != 0
    }

    /// The activation of `ability`, triggered under this one.
    ///
    /// The same wherever it's derived, so a client and its server agree on it
    /// without asking. Derived from the server's, it's the server's.
    #[must_use]
    pub const fn derived(self, ability: AbilityId) -> Self {
        // SplitMix64 over the parent and the ability: stable everywhere.
        let mut mixed = self.0 ^ (ability.index() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        mixed ^= mixed >> 31;
        Self((mixed & COUNTER) | DERIVED | (self.0 & SERVERS))
    }
}

/// Whose it is and its count: `client#7`, `server#7`, `derived#…` or
/// `server-derived#…`.
impl fmt::Display for Activation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.0 & COUNTER;
        match (self.is_servers(), self.is_derived()) {
            (false, false) => write!(f, "client#{count}"),
            (true, false) => write!(f, "server#{count}"),
            (false, true) => write!(f, "derived#{count:x}"),
            (true, true) => write!(f, "server-derived#{count:x}"),
        }
    }
}

/// As it's displayed.
impl fmt::Debug for Activation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::Activation;
    use crate::registry::AbilityId;

    #[test]
    fn a_clients_number_cant_set_a_flag() {
        let activation = Activation::client(u64::MAX);
        assert!(!activation.is_servers());
        assert!(!activation.is_derived());
        assert!(Activation::server(u64::MAX).is_servers());
        assert!(!Activation::server(u64::MAX).is_derived());
    }

    #[test]
    fn a_derived_activation_is_the_same_wherever_its_derived() {
        let ability = AbilityId::new(3);
        let parent = Activation::client(7);
        assert_eq!(parent.derived(ability), parent.derived(ability));
    }

    #[test]
    fn a_derived_activation_differs_by_parent_and_by_ability() {
        let ability = AbilityId::new(3);
        let parent = Activation::client(7);
        assert_ne!(
            parent.derived(ability),
            Activation::client(8).derived(ability),
            "by parent"
        );
        assert_ne!(
            parent.derived(ability),
            parent.derived(AbilityId::new(4)),
            "by ability"
        );
    }

    #[test]
    fn a_derived_activation_is_derived_and_keeps_its_parents_side() {
        let ability = AbilityId::new(3);
        let derived = Activation::client(5).derived(AbilityId::new(1));
        let parents = [0, 1, 7, u64::from(u32::MAX)]
            .map(Activation::client)
            .into_iter()
            .chain([derived]);
        for parent in parents {
            let clients = parent.derived(ability);
            assert!(clients.is_derived(), "derived, from {parent:?}");
            assert!(!clients.is_servers(), "a client's, from {parent:?}");
        }
        let servers = [0, 1, 7, u64::from(u32::MAX)]
            .map(Activation::server)
            .into_iter()
            .chain([Activation::server(5).derived(AbilityId::new(1))]);
        for parent in servers {
            let derived = parent.derived(ability);
            assert!(derived.is_derived(), "derived, from {parent:?}");
            assert!(derived.is_servers(), "the server's, from {parent:?}");
        }
    }

    #[test]
    fn a_derived_activation_derived_again_is_another() {
        let ability = AbilityId::new(3);
        let child = Activation::client(7).derived(ability);
        assert_ne!(child.derived(ability), child);
    }

    #[test]
    fn it_says_whose_it_is() {
        assert_eq!(Activation::client(7).to_string(), "client#7");
        assert_eq!(format!("{:?}", Activation::server(7)), "server#7");
        let derived = Activation::server(7).derived(AbilityId::new(3));
        assert!(format!("{derived:?}").starts_with("server-derived#"));
    }

    #[test]
    fn its_written_as_its_number() {
        use serde_test::{Token, assert_tokens};

        assert_tokens(&Activation::client(7), &[Token::U64(7)]);
        assert_tokens(&Activation::server(7), &[Token::U64((1 << 63) | 7)]);
    }
}
