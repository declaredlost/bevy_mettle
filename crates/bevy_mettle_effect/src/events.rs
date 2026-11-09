//! What's told as effects apply, execute, stack and come off.
//!
//! What cues and abilities observe.

use bevy::prelude::*;
use bevy_mettle_attribute::registry::AttributeId;
use serde::{Deserialize, Serialize};

use crate::EffectId;
use crate::code::RouteId;

/// An effect applied to an actor, lasting: its active effect spawned.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectApplied {
    /// The actor.
    pub entity: Entity,
    /// The active effect.
    pub effect: Entity,
    /// Which effect.
    pub id: EffectId,
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
    pub id: EffectId,
    /// How many stacks it has now.
    pub stacks: u32,
}

/// Triggered on an actor as an effect fails to apply to it, with why.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectBlocked {
    /// The actor.
    pub entity: Entity,
    /// Which effect.
    pub id: EffectId,
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

/// What an execution changed one attribute's base by, such as damage taken.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// The attribute.
    pub attribute: AttributeId,
    /// What its base was changed by, rounded and kept within its limits:
    /// `-30.0`, damage taken.
    pub delta: f32,
}

/// What an execution dealt one route, before its router turned it into changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Routed {
    /// The route.
    pub route: RouteId,
    /// All the execution dealt it.
    pub amount: f32,
}

/// Triggered as an effect executes on an actor, its bases changed.
///
/// An instant one applied, or a periodic one's period come round: what it
/// changed, on whom, for a combat log or a reaction.
#[derive(EntityEvent, Debug, Clone, PartialEq)]
pub struct EffectExecuted {
    /// The actor.
    pub entity: Entity,
    /// Which effect.
    pub id: EffectId,
    /// Who applied it, if anyone.
    pub source: Option<Entity>,
    /// What it changed the actor's bases by, each attribute once, in the
    /// order it first changed them.
    pub changes: Vec<Delta>,
    /// What it changed its source's bases by: life stolen back.
    pub source_changes: Vec<Delta>,
    /// What each route was dealt, before its router: a hit's damage before
    /// shields took their share.
    pub routed: Vec<Routed>,
}

/// An active effect taken off an actor: run out of time, or removed.
///
/// Whether it ran out of time is `expired`.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct EffectRemoved {
    /// The actor.
    pub entity: Entity,
    /// The active effect, now despawned.
    pub effect: Entity,
    /// Which effect.
    pub id: EffectId,
    /// Whether it ran out of time.
    pub expired: bool,
}
