#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod ability;
pub mod active;
pub mod behaviour;
mod commands;
pub mod data;
mod engine;
pub mod events;
pub mod net;
mod plugin;
pub mod prelude;
mod reader;
mod registry;

/// Core, as the derives reach it wherever this crate's used.
#[doc(hidden)]
pub mod __core {
    pub use bevy_mettle_core::*;
}

pub use self::ability::{Ability, AbilityApp, AbilitySpec, IntoAbilityId};
pub use self::commands::{AbilityCommands, AbilityWorld};
pub use self::plugin::{AbilityPlugin, AbilitySystems};
pub use self::reader::AbilityReader;
pub use self::registry::{AbilityChecks, AbilityRegistry};
