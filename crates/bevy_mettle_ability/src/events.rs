//! Gameplay events, and what's told as abilities activate and end.

use bevy::prelude::*;
use bevy_mettle_core::tag::TagKey;

use crate::AbilityId;

/// A gameplay event sent to an actor: a tag, and who and how much.
///
/// What abilities with an `Event` trigger activate on, with it; and an entity
/// event of its own, for the game to observe. Unreal's
/// `SendGameplayEventToActor`.
#[derive(EntityEvent, Debug, Clone, PartialEq)]
pub struct GameplayEvent {
    /// The actor it's sent to.
    pub entity: Entity,
    /// What it is: `event.hit.parried`.
    pub tag: TagKey,
    /// Who caused it.
    pub instigator: Option<Entity>,
    /// Who it's about.
    pub target: Option<Entity>,
    /// How much.
    pub magnitude: f32,
}

impl GameplayEvent {
    /// `tag`, sent to `entity`, by no one, of nothing.
    #[must_use]
    pub fn new(entity: Entity, tag: impl Into<TagKey>) -> Self {
        Self {
            entity,
            tag: tag.into(),
            instigator: None,
            target: None,
            magnitude: 0.0,
        }
    }
}

/// Triggered on an actor as one of its abilities activates.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityActivated {
    /// The actor.
    pub entity: Entity,
    /// The granted ability.
    pub ability: Entity,
    /// Which ability.
    pub id: AbilityId,
    /// The activation.
    pub activation: u64,
}

/// Triggered on an actor as an ability it was asked for didn't activate.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityFailed {
    /// The actor.
    pub entity: Entity,
    /// Which ability.
    pub id: AbilityId,
    /// Why.
    pub reason: Failure,
}

/// Why an ability didn't activate, or couldn't commit: its cost, a cooldown.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize
)]
pub enum Failure {
    /// It isn't granted to the actor.
    NotGranted,
    /// It's active already.
    Active,
    /// It's cooling down.
    Cooldown,
    /// Its cost would take an attribute below 0.
    Cost,
    /// Its owner owns what it's blocked by, or an active ability blocks it.
    Blocked,
    /// Its requirement doesn't hold.
    Requirement,
    /// What interrupts it holds: its `cancel_if`.
    Interrupted,
    /// Its input wasn't given, or isn't what its behaviour takes, or doesn't
    /// pass its check: at activation, or given while it was active.
    Input,
}

/// Triggered on an actor as one of its abilities ends, or is cancelled.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityEnded {
    /// The actor.
    pub entity: Entity,
    /// The granted ability.
    pub ability: Entity,
    /// Which ability.
    pub id: AbilityId,
    /// Whether it was cancelled.
    pub cancelled: bool,
}
