//! What using attributes takes: `use bevy_mettle_attribute::prelude::*`.

#[doc(hidden)]
pub use bevy_mettle_core::Id;
#[doc(hidden)]
pub use bevy_mettle_core::condition::Condition;
#[doc(hidden)]
pub use bevy_mettle_core::tag::{OwnedTags, Tag, TagAdded, TagCounts, TagRemoved, TagTree, Tags};

#[doc(hidden)]
pub use crate::modifier::{Modifier, ModifierOf, ModifierSources, Modifiers, Operation};
#[doc(hidden)]
pub use crate::read::{AttributeContext, AttributeReader, AttributeWorld, Breakdown};
#[doc(hidden)]
pub use crate::registry::{AttributeChanged, AttributeRegistry, Limit, Rounding};
#[doc(hidden)]
pub use crate::{Attribute, AttributeApp, AttributePlugin, AttributeSystems, Base, Current};
