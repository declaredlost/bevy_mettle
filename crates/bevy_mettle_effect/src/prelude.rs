//! What using effects takes: `use bevy_mettle_effect::prelude::*`.
//!
//! Attributes are `bevy_mettle_attribute`'s, and tags, curves and conditions
//! `bevy_mettle_core`'s, each in its own prelude. Naming executions and
//! calculations by id takes [`Execution`](crate::code::Execution) and
//! [`Calculation`](crate::code::Calculation) too.

#[doc(no_inline)]
pub use crate::active::{ActiveEffect, ActiveEffects, EffectOf};
#[doc(no_inline)]
pub use crate::code::{BaseChange, EffectCode, EffectCodeApp, EffectContext};
#[doc(no_inline)]
pub use crate::cue::{
    ActiveCue, ActiveCues, Cue, CueAdded, CueApp, CueCommands, CueEvent, CueExecuted, CueOf,
    CueRemoved, CueSystems, CueWorld,
};
#[doc(no_inline)]
pub use crate::data::{
    Capture, Effect, EffectDuration, EffectModifier, Expire, Magnitude, Reapply, StackBy, Stacking,
};
#[doc(no_inline)]
pub use crate::events::{
    Blocked, EffectApplied, EffectBlocked, EffectExecuted, EffectRemoved, EffectStacked,
};
#[doc(no_inline)]
pub use crate::net::{EffectReplica, PredictedUnder, Proxy};
#[doc(no_inline)]
pub use crate::{
    EffectChecks, EffectCommands, EffectPlugin, EffectRegistry, EffectSpec, EffectSystems,
    EffectWorld,
};
