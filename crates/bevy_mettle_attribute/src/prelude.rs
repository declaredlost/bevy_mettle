//! What using attributes takes: `use bevy_mettle_attribute::prelude::*`.
//!
//! Tags, curves and conditions are `bevy_mettle_core`'s, in its own prelude.
//! Implementing [`Attribute`] by hand takes
//! [`AnyAttribute`](crate::registry::AnyAttribute) too.

#[doc(no_inline)]
pub use crate::authority::{Remote, Served};
#[doc(no_inline)]
pub use crate::modifier::{
    Modifier, ModifierOf, ModifierSources, Modifiers, Operation, Provisional,
};
#[doc(no_inline)]
pub use crate::read::{
    AttributeContext, AttributeReader, AttributeWorld, Breakdown, attribute_above, attribute_below,
};
#[doc(no_inline)]
pub use crate::registry::{AttributeApp, AttributeChanged, AttributeRegistry, Limit, Rounding};
#[doc(no_inline)]
pub use crate::{Attribute, AttributePlugin, AttributeSystems, Base, Current};
