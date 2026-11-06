#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

pub mod prelude;

#[cfg(feature = "ability")]
pub use bevy_mettle_ability as ability;
#[cfg(feature = "attribute")]
pub use bevy_mettle_attribute as attribute;
pub use bevy_mettle_core as core;
#[cfg(feature = "effect")]
pub use bevy_mettle_effect as effect;
