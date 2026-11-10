//! Keys: the names content defines things by, typed by their kind.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy_reflect::prelude::*;
use bevy_reflect::utility::GenericTypePathCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

use super::Kind;

/// The key of something content defines: its name, typed by its [`Kind`].
///
/// What content names it by, and what's shown for it.
///
/// A `Key<AnyEffect>` can't stand in for a `Key<AnyAbility>`. Once content
/// is checked, it's known by its number instead, an [`Id`](super::Id) of the
/// same kind.
///
/// Cheap to clone: a name up to 23 bytes is held inline, a longer one shared.
/// Written in data as its name (`"fire_resistance"`), and reflected as a value.
#[derive(Reflect)]
#[reflect(opaque, type_path = false)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Key<T: Kind> {
    name: SmolStr,
    kind: PhantomData<fn() -> T>,
}

impl<T: Kind> Key<T> {
    /// The key named `name`.
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            name: name.into(),
            kind: PhantomData,
        }
    }

    /// The key named `name`, as a constant.
    #[must_use]
    pub const fn from_static(name: &'static str) -> Self {
        Self {
            name: SmolStr::new_static(name),
            kind: PhantomData,
        }
    }

    /// Its name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.name
    }
}

/// Named by its kind: `bevy_mettle_core::registry::Key<effect>`.
impl<T: Kind> TypePath for Key<T> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_core::registry::Key<{}>", T::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Key<{}>", T::NAME))
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

impl<T: Kind> Clone for Key<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: PhantomData,
        }
    }
}

impl<T: Kind> PartialEq for Key<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<T: Kind> Eq for Key<T> {}

impl<T: Kind> Hash for Key<T> {
    /// As its name: so a map keyed by names can be looked up by `&str`.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<T: Kind> PartialOrd for Key<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Kind> Ord for Key<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl<T: Kind> Borrow<str> for Key<T> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<T: Kind> fmt::Debug for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({:?})", T::NAME, self.as_str())
    }
}

impl<T: Kind> fmt::Display for Key<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<T: Kind> From<&str> for Key<T> {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl<T: Kind> From<String> for Key<T> {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl<T: Kind> Serialize for Key<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name.serialize(serializer)
    }
}

impl<'de, T: Kind> Deserialize<'de> for Key<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        SmolStr::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::registry::AnyCurve;

    #[test]
    fn a_key_looks_up_by_its_name() {
        let mut map = HashMap::new();
        map.insert(Key::<AnyCurve>::from_static("fire"), 1);
        assert_eq!(map.get("fire"), Some(&1));
    }

    #[test]
    fn a_key_says_its_kind() {
        let fire: Key<AnyCurve> = "fire".into();
        assert_eq!(format!("{fire:?}"), r#"curve("fire")"#);
        assert_eq!(fire.to_string(), "fire");
    }
}
