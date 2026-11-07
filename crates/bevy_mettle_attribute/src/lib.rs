#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod attribute;
pub mod authority;
pub mod modifier;
mod plugin;
pub mod prelude;
pub mod read;
pub mod registry;
mod values;

pub use bevy_mettle_macros::Attribute;

pub use self::attribute::Attribute;
pub use self::plugin::{AttributePlugin, AttributeSystems};
pub use self::registry::AttributeApp;
pub use self::values::{Base, Current};
