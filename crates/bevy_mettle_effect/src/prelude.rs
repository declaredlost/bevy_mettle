//! What using effects takes: `use bevy_mettle_effect::prelude::*`.

#[doc(hidden)]
pub use crate::active::{ActiveEffect, ActiveEffects, EffectOf};
#[doc(hidden)]
pub use crate::code::{BaseChange, EffectCode, EffectContext};
#[doc(hidden)]
pub use crate::data::{
    Capture, EffectDuration, EffectModifier, Expire, Magnitude, Reapply, StackBy, Stacking,
};
#[doc(hidden)]
pub use crate::events::{
    Blocked, EffectApplied, EffectBlocked, EffectExecuted, EffectRemoved, EffectStacked,
};
#[doc(hidden)]
pub use crate::net::{EffectReplica, NetRole, PredictedUnder, Proxy};
#[doc(hidden)]
pub use crate::{
    Effect, EffectCommands, EffectPlugin, EffectRegistry, EffectSpec, EffectSystems, EffectWorld,
};
