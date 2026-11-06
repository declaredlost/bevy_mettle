//! Gameplay events, and what's told as abilities activate and end.

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::Tag;

use crate::Ability;

/// A gameplay event sent to an actor: a tag, and who and how much. What
/// abilities with an `Event` trigger activate on, with it; and an entity
/// event of its own, for the game to observe. Unreal's
/// `SendGameplayEventToActor`.
#[derive(EntityEvent, Debug, Clone, PartialEq)]
pub struct GameplayEvent {
    /// The actor it's sent to.
    pub entity: Entity,
    /// What it is: `event.hit.parried`.
    pub tag: Id<Tag>,
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
    pub fn new(entity: Entity, tag: impl Into<Id<Tag>>) -> Self {
        Self {
            entity,
            tag: tag.into(),
            instigator: None,
            target: None,
            magnitude: 0.0,
        }
    }
}

/// An ability activated for an actor.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityActivated {
    /// The actor.
    pub entity: Entity,
    /// The granted ability.
    pub ability: Entity,
    /// Which ability.
    pub id: Id<Ability>,
    /// The activation.
    pub activation: u64,
}

/// An ability that didn't activate for an actor.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityFailed {
    /// The actor.
    pub entity: Entity,
    /// Which ability.
    pub id: Id<Ability>,
    /// Why.
    pub reason: Failure,
}

/// Why an ability didn't activate, or couldn't commit.
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
}

/// An ability that was active for an actor, ended: by its behaviour, or
/// cancelled.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct AbilityEnded {
    /// The actor.
    pub entity: Entity,
    /// The granted ability.
    pub ability: Entity,
    /// Which ability.
    pub id: Id<Ability>,
    /// Whether it was cancelled.
    pub cancelled: bool,
}
