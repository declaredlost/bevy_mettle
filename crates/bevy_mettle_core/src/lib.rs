#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

// The derive names this crate by its path, here too.
extern crate self as bevy_mettle_core;

mod activation;
pub mod condition;
pub mod curve;
mod error;
pub mod net;
pub mod prelude;
pub mod registry;
mod schedule;
pub mod tag;

pub use self::activation::Activation;
pub use self::error::{Error, Errors};
pub use self::schedule::{Mettle, MettleCorePlugin, MettleSet, MettleSystems};
