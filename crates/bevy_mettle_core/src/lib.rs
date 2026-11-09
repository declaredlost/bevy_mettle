#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

// The derive names this crate by its path, here too.
extern crate self as bevy_mettle_core;

pub mod condition;
pub mod curve;
mod error;
mod id;
pub mod net;
#[doc(hidden)]
pub mod optional;
pub mod prelude;
pub mod tag;

pub use bevy_mettle_macros::IdTarget;

pub use self::error::{MettleError, MettleErrors};
pub use self::id::{Id, IdTarget, Number, Numbering, Registry};

/// What [`number!`] expands to needs, wherever it's used.
#[doc(hidden)]
pub mod __private {
    pub use bevy_reflect::Reflect;
    pub use serde;
}
