#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod active;
pub mod code;
mod commands;
pub mod context;
pub mod cue;
pub mod data;
mod engine;
pub mod events;
pub mod net;
mod plugin;
pub mod prelude;
mod registry;
mod snapshot;
mod spec;
mod systems;

/// Core, as the derives reach it wherever this crate's used.
#[doc(hidden)]
pub mod __core {
    pub use bevy_mettle_core::*;
}

pub use self::commands::{EffectCommands, EffectWorld};
pub use self::plugin::EffectPlugin;
pub use self::registry::{EffectChecks, EffectRegistry};
pub use self::snapshot::Snapshots;
pub use self::spec::{Effect, EffectSpec, IntoEffectId, Snapshot};
