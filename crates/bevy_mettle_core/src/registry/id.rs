//! Ids: what an app knows content and code by, as Bevy knows a component by
//! its `ComponentId`, and each core kind's id.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy_reflect::prelude::*;
use bevy_reflect::utility::GenericTypePathCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{AnyAbility, AnyAttribute, AnyCurve, AnyEffect, AnyTag, Kind};

/// Something's dense number in its [`Registry`](super::Registry), typed by its
/// kind `K`.
///
/// What an app's state, its events and the network know it by, as Bevy knows a
/// component by its `ComponentId`.
///
/// Each kind has its own: `EffectId` is `Id<AnyEffect>`, so an effect's
/// number can't stand in for an ability's. Its name, a [`Key`](super::Key)
/// of the same kind, lives beside it in the registry. Written as its number,
/// and reflected as a value.
#[derive(Reflect)]
#[reflect(opaque, type_path = false)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Id<K: Kind> {
    index: u32,
    kind: PhantomData<fn() -> K>,
}

impl<K: Kind> Id<K> {
    /// The id numbered `index`.
    #[must_use]
    pub const fn new(index: u32) -> Self {
        Self {
            index,
            kind: PhantomData,
        }
    }

    /// Its index: where it's kept, in its registry or an actor's state.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index as usize
    }
}

/// Named by its kind: `bevy_mettle_core::registry::Id<effect>`.
impl<K: Kind> TypePath for Id<K> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_core::registry::Id<{}>", K::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Id<{}>", K::NAME))
    }

    fn type_ident() -> Option<&'static str> {
        Some("Id")
    }

    fn crate_name() -> Option<&'static str> {
        Some("bevy_mettle_core")
    }

    fn module_path() -> Option<&'static str> {
        Some("bevy_mettle_core::registry")
    }
}

impl<K: Kind> Clone for Id<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Kind> Copy for Id<K> {}

impl<K: Kind> PartialEq for Id<K> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<K: Kind> Eq for Id<K> {}

impl<K: Kind> Hash for Id<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<K: Kind> PartialOrd for Id<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Kind> Ord for Id<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

/// As its kind and number: `effect#3`.
impl<K: Kind> fmt::Debug for Id<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", K::NAME, self.index)
    }
}

impl<K: Kind> Serialize for Id<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.index.serialize(serializer)
    }
}

impl<'de, K: Kind> Deserialize<'de> for Id<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u32::deserialize(deserializer).map(Self::new)
    }
}

/// A tag's number in the [`TagRegistry`](crate::tag::TagRegistry): what the
/// network sends for it.
///
/// The same on a server and its clients, as they check the same tags. A
/// [`TagKey`](crate::tag::TagKey) is a tag as this process knows it; a
/// `TagId` is its place in the registry, shared.
pub type TagId = Id<AnyTag>;

/// A curve's number in the [`CurveRegistry`](crate::curve::CurveRegistry):
/// what magnitudes scale by.
///
/// Magnitudes name a curve by number once content is checked.
pub type CurveId = Id<AnyCurve>;

/// An attribute's number in its registry, given as it's added to an app.
///
/// As Bevy's `ComponentId` is to a component: what a modifier, an effect or a
/// replica knows it by.
pub type AttributeId = Id<AnyAttribute>;

/// An effect's number in its registry, as the app knows it.
///
/// What an active effect, a replica, a chain or an event knows it by, as Bevy
/// knows a component by its `ComponentId`.
///
/// Numbered in order of name as content is checked, so the same content is
/// numbered the same on a server and its clients; a reload keeps the
/// numbers effects had.
pub type EffectId = Id<AnyEffect>;

/// An ability's number in its registry, as the app knows it.
///
/// What a granted ability, a request, a replica, an event or an effect's
/// context knows it by.
///
/// Numbered in order of name as content is checked, so the same content is
/// numbered the same on a server and its clients; a reload keeps the
/// numbers abilities had.
pub type AbilityId = Id<AnyAbility>;

#[cfg(test)]
mod tests {
    use serde_test::{Token, assert_tokens};

    use super::*;

    #[derive(crate::registry::Kind)]
    enum AnySword {}

    #[test]
    fn an_id_says_its_kind_and_number() {
        let id = Id::<AnySword>::new(3);
        assert_eq!(id.index(), 3);
        assert_eq!(format!("{id:?}"), "sword#3");
        assert_tokens(&id, &[Token::U32(3)]);
    }
}
