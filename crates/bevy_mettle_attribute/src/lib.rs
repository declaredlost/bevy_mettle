#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod attribute;
mod context;
mod events;
pub mod guards;
mod modifier;
mod plugin;
pub mod prelude;
mod reader;
mod registry;
mod state;
mod values;

pub use bevy_mettle_macros::Attribute;

pub use self::attribute::{AnyAttribute, Attribute, Limit, Rounding};
pub use self::context::AttributeContext;
pub use self::events::AttributeChanged;
pub use self::modifier::{Modifier, ModifierOf, ModifierSources, Modifiers, Operation, Per};
pub use self::plugin::{AttributePlugin, AttributeSystems};
pub use self::reader::{AttributeReader, AttributeWorld};
pub use self::registry::{AttributeApp, AttributeInfo, AttributeRegistry};
pub use self::state::{AttributeState, Breakdown};
pub use self::values::{Base, Current};
