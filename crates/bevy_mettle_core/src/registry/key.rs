//! Keys: what an app knows content and code by, as Bevy knows a component by
//! its `ComponentId`.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy_reflect::prelude::*;
use bevy_reflect::utility::GenericTypePathCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Kind;

/// Something's dense number in its [`Registry`](super::Registry), typed by its
/// kind `K`.
///
/// What an app's state, its events and the network know it by, as Bevy knows a
/// component by its `ComponentId`.
///
/// Each kind has its own: `EffectId` is `Key<AnyEffect>`, so an effect's
/// number can't stand in for an ability's. Its name, a
/// [`Name`](super::Name) of the same kind, lives beside it in the
/// registry. Written as its number, and reflected as a value.
#[derive(Reflect)]
#[reflect(opaque, type_path = false)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Key<K: Kind> {
    index: u32,
    kind: PhantomData<fn() -> K>,
}

impl<K: Kind> Key<K> {
    /// The key numbered `index`.
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

/// Named by its kind: `bevy_mettle_core::registry::Key<effect>`.
impl<K: Kind> TypePath for Key<K> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_core::registry::Key<{}>", K::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Key<{}>", K::NAME))
    }

    fn type_ident() -> Option<&'static str> {
        Some("Key")
    }

    fn crate_name() -> Option<&'static str> {
        Some("bevy_mettle_core")
    }

    fn module_path() -> Option<&'static str> {
        Some("bevy_mettle_core::registry")
    }
}

impl<K: Kind> Clone for Key<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Kind> Copy for Key<K> {}

impl<K: Kind> PartialEq for Key<K> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<K: Kind> Eq for Key<K> {}

impl<K: Kind> Hash for Key<K> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<K: Kind> PartialOrd for Key<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Kind> Ord for Key<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.index.cmp(&other.index)
    }
}

/// As its kind and number: `effect#3`.
impl<K: Kind> fmt::Debug for Key<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", K::NAME, self.index)
    }
}

impl<K: Kind> Serialize for Key<K> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.index.serialize(serializer)
    }
}

impl<'de, K: Kind> Deserialize<'de> for Key<K> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u32::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use serde_test::{Token, assert_tokens};

    use super::*;

    #[derive(crate::registry::Kind)]
    enum AnySword {}

    #[test]
    fn a_key_says_its_kind_and_number() {
        let key = Key::<AnySword>::new(3);
        assert_eq!(key.index(), 3);
        assert_eq!(format!("{key:?}"), "sword#3");
        assert_tokens(&key, &[Token::U32(3)]);
    }
}
