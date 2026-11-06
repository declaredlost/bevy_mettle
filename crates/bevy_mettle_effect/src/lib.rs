#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

mod active;
mod code;
mod commands;
mod effect;
mod engine;
mod events;
mod net;
mod plugin;
pub mod prelude;
mod registry;
mod spec;
mod systems;

pub use self::active::{ActiveEffect, ActiveEffects, EffectOf};
pub use self::code::{
    BaseChange, Calculation, CalculationFn, EffectCode, EffectContext, Execution, ExecutionFn,
};
pub use self::commands::{EffectCommands, EffectWorld};
pub use self::effect::{
    Capture, Effect, EffectDuration, EffectModifier, Expire, Magnitude, Period, Reapply, StackBy,
    Stacking,
};
pub use self::events::{
    Blocked, EffectApplied, EffectBlocked, EffectExecuted, EffectRemoved, EffectStacked,
};
pub use self::net::{EffectReplica, NetRole, PredictedUnder, Proxy};
pub use self::plugin::{EffectPlugin, EffectSystems};
pub use self::registry::EffectRegistry;
pub use self::spec::EffectSpec;
