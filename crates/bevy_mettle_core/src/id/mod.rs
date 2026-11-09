//! Ids: what everything defined in data is named by, in data and in code alike,
//! and the kinds of thing they name.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy_reflect::prelude::*;
use bevy_reflect::utility::GenericTypePathCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

pub use self::numbering::Numbering;
pub use self::registry::{Number, Registry};
pub use self::target::IdTarget;

mod number;
mod numbering;
mod registry;
mod target;

/// The id of something defined in data: its name, typed by what it names.
///
/// Something of [`IdTarget`](trait@IdTarget) `T`, so a tag's id can't stand in
/// for an item's.
///
/// Cheap to clone: a name up to 23 bytes is held inline, a longer one shared.
/// Written in data as its name (`"fire_resistance"`), and reflected as a value.
#[derive(Reflect)]
#[reflect(opaque, type_path = false)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Id<T: IdTarget> {
    name: SmolStr,
    kind: PhantomData<fn() -> T>,
}

impl<T: IdTarget> Id<T> {
    /// The id called `name`.
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            name: name.into(),
            kind: PhantomData,
        }
    }

    /// The id called `name`, as a constant.
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

/// Named by what it names: `bevy_mettle_core::Id<tag>`.
impl<T: IdTarget> TypePath for Id<T> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_core::Id<{}>", T::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Id<{}>", T::NAME))
    }

    fn type_ident() -> Option<&'static str> {
        Some("Id")
    }

    fn crate_name() -> Option<&'static str> {
        Some("bevy_mettle_core")
    }

    fn module_path() -> Option<&'static str> {
        Some("bevy_mettle_core")
    }
}

impl<T: IdTarget> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: PhantomData,
        }
    }
}

impl<T: IdTarget> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<T: IdTarget> Eq for Id<T> {}

impl<T: IdTarget> Hash for Id<T> {
    /// As its name: so a map keyed by ids can be looked up by `&str`.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<T: IdTarget> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: IdTarget> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl<T: IdTarget> Borrow<str> for Id<T> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<T: IdTarget> fmt::Debug for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({:?})", T::NAME, self.as_str())
    }
}

impl<T: IdTarget> fmt::Display for Id<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<T: IdTarget> From<&str> for Id<T> {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl<T: IdTarget> From<String> for Id<T> {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl<T: IdTarget> Serialize for Id<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name.serialize(serializer)
    }
}

impl<'de, T: IdTarget> Deserialize<'de> for Id<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        SmolStr::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::curve::AnyCurve;

    #[test]
    fn an_id_looks_up_by_its_name() {
        let mut map = HashMap::new();
        map.insert(Id::<AnyCurve>::from_static("fire"), 1);
        assert_eq!(map.get("fire"), Some(&1));
    }

    #[test]
    fn an_id_says_what_it_names() {
        let fire: Id<AnyCurve> = "fire".into();
        assert_eq!(format!("{fire:?}"), r#"curve("fire")"#);
        assert_eq!(fire.to_string(), "fire");
    }
}
