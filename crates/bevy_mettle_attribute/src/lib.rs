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

/// Core, as the derives reach it wherever this crate's used.
#[doc(hidden)]
pub mod __core {
    pub use bevy_mettle_core::*;
}

pub use bevy_mettle_macros::Attribute;

pub use self::attribute::Attribute;
pub use self::plugin::{AttributeFold, AttributePlugin};
pub use self::values::{Base, Current};
