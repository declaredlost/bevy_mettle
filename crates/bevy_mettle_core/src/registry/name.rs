//! Names: what content names what it defines by, typed by its kind.

use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;

use bevy_reflect::prelude::*;
use bevy_reflect::utility::GenericTypePathCell;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::SmolStr;

/// A kind of thing content defines and names: an effect, an ability, a curve.
///
/// A tag is a kind too, as is anything else a game defines.
///
/// What a [`Name`] names, and a [`Key`](super::Key) numbers. Usually
/// an empty enum, `AnyEffect`. Derive it, named in messages as the type is in
/// `snake_case`, without a leading `Any`, unless `#[kind(name = "..")]` says
/// otherwise:
///
/// ```
/// use bevy_mettle_core::registry::{Kind, Name};
///
/// #[derive(Kind)]
/// enum AnyItem {}
///
/// let sword: Name<AnyItem> = Name::from_static("sword");
/// assert_eq!(format!("{sword:?}"), r#"item("sword")"#);
/// ```
pub trait Kind: 'static {
    /// What one is called in messages: `effect`, `item`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::Kind;

/// The name of something content defines, typed by its [`Kind`], and shown for
/// it.
///
/// What content names it by, and what's shown for it.
///
/// A `Name<AnyEffect>` can't stand in for a `Name<AnyAbility>`. Once content
/// is checked, it's known by its number instead, a
/// [`Key`](super::Key) of the same kind.
///
/// Cheap to clone: a name up to 23 bytes is held inline, a longer one shared.
/// Written in data as its name (`"fire_resistance"`), and reflected as a value.
#[derive(Reflect)]
#[reflect(opaque, type_path = false)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub struct Name<T: Kind> {
    name: SmolStr,
    kind: PhantomData<fn() -> T>,
}

impl<T: Kind> Name<T> {
    /// The name `name`.
    pub fn new(name: impl Into<SmolStr>) -> Self {
        Self {
            name: name.into(),
            kind: PhantomData,
        }
    }

    /// The name `name`, as a constant.
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

/// Named by its kind: `bevy_mettle_core::registry::Name<effect>`.
impl<T: Kind> TypePath for Name<T> {
    fn type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("bevy_mettle_core::Name<{}>", T::NAME))
    }

    fn short_type_path() -> &'static str {
        static CELL: GenericTypePathCell = GenericTypePathCell::new();
        CELL.get_or_insert::<Self, _>(|| format!("Name<{}>", T::NAME))
    }

    fn type_ident() -> Option<&'static str> {
        Some("Name")
    }

    fn crate_name() -> Option<&'static str> {
        Some("bevy_mettle_core")
    }

    fn module_path() -> Option<&'static str> {
        Some("bevy_mettle_core::registry")
    }
}

impl<T: Kind> Clone for Name<T> {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            kind: PhantomData,
        }
    }
}

impl<T: Kind> PartialEq for Name<T> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl<T: Kind> Eq for Name<T> {}

impl<T: Kind> Hash for Name<T> {
    /// As its name: so a map keyed by names can be looked up by `&str`.
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<T: Kind> PartialOrd for Name<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Kind> Ord for Name<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name.cmp(&other.name)
    }
}

impl<T: Kind> Borrow<str> for Name<T> {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl<T: Kind> fmt::Debug for Name<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}({:?})", T::NAME, self.as_str())
    }
}

impl<T: Kind> fmt::Display for Name<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl<T: Kind> From<&str> for Name<T> {
    fn from(name: &str) -> Self {
        Self::new(name)
    }
}

impl<T: Kind> From<String> for Name<T> {
    fn from(name: String) -> Self {
        Self::new(name)
    }
}

impl<T: Kind> Serialize for Name<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name.serialize(serializer)
    }
}

impl<'de, T: Kind> Deserialize<'de> for Name<T> {
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
    fn a_name_looks_up_by_its_str() {
        let mut map = HashMap::new();
        map.insert(Name::<AnyCurve>::from_static("fire"), 1);
        assert_eq!(map.get("fire"), Some(&1));
    }

    #[test]
    fn a_name_says_its_kind() {
        let fire: Name<AnyCurve> = "fire".into();
        assert_eq!(format!("{fire:?}"), r#"curve("fire")"#);
        assert_eq!(fire.to_string(), "fire");
    }
}
