//! What using attributes takes.

pub use bevy_mettle_core::Id;
pub use bevy_mettle_core::condition::Condition;
pub use bevy_mettle_core::tag::{OwnedTags, Tag, TagAdded, TagCounts, TagRemoved, TagTree, Tags};

pub use crate::{
    Attribute, AttributeApp, AttributeChanged, AttributeContext, AttributePlugin, AttributeReader,
    AttributeRegistry, AttributeSystems, AttributeWorld, Base, Breakdown, Current, Limit, Modifier,
    ModifierOf, ModifierSources, Modifiers, Operation, Rounding,
};
