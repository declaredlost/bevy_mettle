//! What using effects takes: defining, applying and observing them, and cues.
//!
//! `use bevy_mettle_effect::prelude::*`.
//!
//! Attributes are `bevy_mettle_attribute`'s, and tags, curves and conditions
//! `bevy_mettle_core`'s, each in its own prelude. What content is checked by
//! (registries and scopes), and what's replicated, are reached by their
//! modules.

#[doc(no_inline)]
pub use crate::active::{ActiveEffect, ActiveEffects, EffectOf};
#[doc(no_inline)]
pub use crate::code::{
    BaseChange, Calculation, CalculationFn, EffectApp, Execution, ExecutionContext, ExecutionFn,
    Route, RouteFn,
};
#[doc(no_inline)]
pub use crate::context::{
    AbilityRef, Causer, ContextData, EffectContext, Hit, Origin, ReflectContextData, SourceObject,
};
#[doc(no_inline)]
pub use crate::cue::{
    ActiveCue, ActiveCues, Cue, CueCommands, CueExecuted, CueOf, CueSystems, CueWorld,
};
#[doc(no_inline)]
pub use crate::data::{
    AttributeRead, EffectData, EffectDuration, EffectModifier, Expire, Magnitude, Read, Reapply,
    Side, StackBy, Stacking,
};
#[doc(no_inline)]
pub use crate::events::{
    Blocked, Delta, EffectApplied, EffectBlocked, EffectExecuted, EffectRemoved, EffectStacked,
};
#[doc(no_inline)]
pub use crate::{
    Effect, EffectCommands, EffectPlugin, EffectRegistry, EffectSpec, EffectWorld, Snapshots,
};
