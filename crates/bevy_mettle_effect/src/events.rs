//! What's told as effects apply, execute, stack and come off.
//!
//! What cues and abilities observe.

use bevy::prelude::*;
use bevy_mettle_core::Id;

use crate::Effect;

/// An effect applied to an actor, lasting: its active effect spawned.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectApplied {
    /// The actor.
    pub entity: Entity,
    /// The active effect.
    pub effect: Entity,
    /// Which effect.
    pub id: Id<Effect>,
}

/// Triggered as an effect on an actor gains a stack, or loses one.
///
/// Applied again onto one already on it, or losing a stack as it ran out of
/// time.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectStacked {
    /// The actor.
    pub entity: Entity,
    /// The active effect.
    pub effect: Entity,
    /// Which effect.
    pub id: Id<Effect>,
    /// How many stacks it has now.
    pub stacks: u32,
}

/// Triggered on an actor as an effect fails to apply to it, with why.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectBlocked {
    /// The actor.
    pub entity: Entity,
    /// Which effect.
    pub id: Id<Effect>,
    /// Why.
    pub reason: Blocked,
}

/// Why an effect didn't apply: a requirement, an immunity, or a stack limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Blocked {
    /// Its requirement didn't hold.
    Requirement,
    /// An effect on the actor makes it immune.
    Immunity,
    /// It's at its stacking limit.
    StackLimit,
}

/// Triggered as an effect executes on an actor, its bases changed.
///
/// An instant one applied, or a periodic one's period come round.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectExecuted {
    /// The actor.
    pub entity: Entity,
    /// Which effect.
    pub id: Id<Effect>,
}

/// An active effect taken off an actor: run out of time (`expired`), or
/// removed.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectRemoved {
    /// The actor.
    pub entity: Entity,
    /// The active effect, now despawned.
    pub effect: Entity,
    /// Which effect.
    pub id: Id<Effect>,
    /// Whether it ran out of time.
    pub expired: bool,
}
