#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod active;
mod commands;
pub mod data;
mod engine;
pub mod events;
pub mod net;
mod plugin;
pub mod prelude;
mod registry;

pub use self::commands::{AbilityCommands, AbilityWorld};
pub use self::data::Ability;
pub use self::plugin::AbilityPlugin;
pub use self::registry::AbilityRegistry;
