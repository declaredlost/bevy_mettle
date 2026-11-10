//! What names an entry until it's resolved: its type, or its number.

use std::any::TypeId;
use std::fmt;

use super::{Id, Keyed, Kind};

/// An entry of kind `K` named by its type, as code names it, or its number.
///
/// Mettle's machinery, not a game's: what a spec or a change holds until
/// [`Registry::find`](super::Registry::find) resolves it. Code names
/// entries by type; a number is what mettle has already resolved.
#[doc(hidden)]
pub enum Ref<K: Kind> {
    /// By its type, with its key to warn of it by.
    Type {
        /// The type.
        type_id: TypeId,
        /// Its key.
        key: &'static str,
    },
    /// By its number.
    Id(Id<K>),
}

impl<K: Kind> Ref<K> {
    /// The entry the type `N` names.
    #[must_use]
    pub fn of<N: Keyed<Kind = K> + 'static>() -> Self {
        Self::Type {
            type_id: TypeId::of::<N>(),
            key: N::KEY,
        }
    }

    /// Its number, if it's named by one.
    #[must_use]
    pub fn id(self) -> Option<Id<K>> {
        match self {
            Self::Type { .. } => None,
            Self::Id(id) => Some(id),
        }
    }

    /// What it's called, for a warning: its type's key, or its number.
    #[must_use]
    pub fn label(self) -> String {
        match self {
            Self::Type { key, .. } => key.to_owned(),
            Self::Id(id) => format!("{id:?}"),
        }
    }
}

impl<K: Kind> Clone for Ref<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Kind> Copy for Ref<K> {}

impl<K: Kind> PartialEq for Ref<K> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Type { type_id: a, .. }, Self::Type { type_id: b, .. }) => a == b,
            (Self::Id(a), Self::Id(b)) => a == b,
            _ => false,
        }
    }
}

impl<K: Kind> Eq for Ref<K> {}

impl<K: Kind> fmt::Debug for Ref<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Type { key, .. } => write!(f, "{}({key:?})", K::NAME),
            Self::Id(id) => write!(f, "{id:?}"),
        }
    }
}
