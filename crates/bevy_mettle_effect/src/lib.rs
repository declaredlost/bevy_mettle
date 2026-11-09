#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod active;
pub mod code;
mod commands;
pub mod cue;
pub mod data;
mod engine;
pub mod events;
pub mod net;
mod plugin;
pub mod prelude;
mod registry;
mod spec;
mod systems;

pub use self::commands::{EffectCommands, EffectWorld};
pub use self::plugin::{EffectPlugin, EffectSystems};
pub use self::registry::{EffectChecks, EffectId, EffectRegistry};
pub use self::spec::{Effect, EffectSpec};
