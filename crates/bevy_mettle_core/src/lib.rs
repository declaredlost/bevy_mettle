#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

// The derive names this crate by its path, here too.
extern crate self as bevy_mettle_core;

pub mod condition;
pub mod curve;
mod error;
mod id;
#[doc(hidden)]
pub mod optional;
pub mod prelude;
pub mod tag;

pub use bevy_mettle_macros::IdTarget;

pub use self::error::{MettleError, MettleErrors};
pub use self::id::{Id, IdTarget};
