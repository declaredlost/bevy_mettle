#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

// The derive names this crate by its path, here too.
extern crate self as bevy_mettle_core;

pub mod condition;
pub mod curve;
mod error;
pub mod net;
pub mod prelude;
pub mod registry;
pub mod schedule;
pub mod tag;

pub use self::error::{Error, Errors};
