//! What using attributes takes: their components, modifiers, reads and values.
//!
//! `use bevy_mettle_attribute::prelude::*`.
//!
//! Tags, curves and conditions are `bevy_mettle_core`'s, in its own prelude.
//! Implementing [`Attribute`] by hand takes
//! [`AnyAttribute`](crate::registry::AnyAttribute) too.

#[doc(no_inline)]
pub use crate::authority::{Remote, Served};
#[doc(no_inline)]
pub use crate::modifier::{
    Modifier, ModifierCommands, ModifierOf, ModifierSources, ModifierSpec, Modifiers,
    ModifiersReplica, Operation, Provisional,
};
#[doc(no_inline)]
pub use crate::read::{
    AttributeContext, AttributeReader, AttributeWorld, Breakdown, attribute_above, attribute_below,
};
#[doc(no_inline)]
pub use crate::registry::{
    AttributeApp, AttributeChanged, AttributeId, AttributeRegistry, Limit, OnLimit, Rounding,
};
#[doc(no_inline)]
pub use crate::{Attribute, AttributePlugin, AttributeSystems, Base, Current};
