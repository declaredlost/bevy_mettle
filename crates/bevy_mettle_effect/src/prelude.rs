//! What using effects takes: their components, events, commands, cues and data.
//!
//! `use bevy_mettle_effect::prelude::*`.
//!
//! Attributes are `bevy_mettle_attribute`'s, and tags, curves and conditions
//! `bevy_mettle_core`'s, each in its own prelude. Naming executions and
//! calculations by name takes [`AnyExecution`](crate::code::AnyExecution) and
//! [`AnyCalculation`](crate::code::AnyCalculation) too.

#[doc(no_inline)]
pub use bevy_mettle_core::registry::{AnyEffect, EffectId};

#[doc(no_inline)]
pub use crate::active::{ActiveEffect, ActiveEffects, EffectOf};
#[doc(no_inline)]
pub use crate::code::{
    BaseChange, Calculation, CalculationFn, CalculationRegistry, CodeRegistry, EffectApp,
    Execution, ExecutionContext, ExecutionFn, ExecutionRegistry, Route, RouteFn, RouteRegistry,
};
#[doc(no_inline)]
pub use crate::context::{
    AbilityRef, Causer, ContextData, EffectContext, Hit, Origin, ReflectContextData, SourceObject,
};
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
pub use crate::net::{EffectReplica, PredictedUnder, Proxy};
#[doc(no_inline)]
pub use crate::{
    Effect, EffectCommands, EffectPlugin, EffectRegistry, EffectScope, EffectSpec, EffectWorld,
    Snapshots,
};
