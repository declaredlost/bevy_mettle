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
    /// The activation it was sent under, if an ability running sent it:
    /// what a predicted ability it triggers is predicted under.
    pub activation: Option<u64>,
}

impl GameplayEvent {
    /// `tag`, sent to `entity`, by no one, of nothing, under no activation.
    #[must_use]
    pub fn new(entity: Entity, tag: impl Into<TagKey>) -> Self {
        Self {
            entity,
            tag: tag.into(),
            instigator: None,
            target: None,
            magnitude: 0.0,
            activation: None,
        }
    }

    /// Sent under `activation`: the running ability's, as its behaviour sends
    /// it. A predicted ability it triggers is then predicted too, on the
    /// client and the server alike, its activation derived from this one.
    #[must_use]
    pub fn under(mut self, activation: u64) -> Self {
        self.activation = Some(activation);
        self
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

/// What activating an ability came to: running here, or asked of the server.
///
/// On a client, a server-run ability ([`ServerOnly`], [`ServerInitiated`]) is
/// only asked for: nothing runs yet, and how it went is told later, by an
/// [`AbilityFailed`], an [`AbilityStarted`] (mirrored here) or replicas.
///
/// [`ServerOnly`]: crate::data::NetExecution::ServerOnly
/// [`ServerInitiated`]: crate::data::NetExecution::ServerInitiated
/// [`AbilityStarted`]: crate::net::start::AbilityStarted
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Activated {
    /// Running here: the granted ability, active.
    Running(Entity),
    /// Asked of the server: the granted ability, not active here.
    Asked(Entity),
}

impl Activated {
    /// The granted ability, running or asked for.
    #[must_use]
    pub fn ability(self) -> Entity {
        match self {
            Self::Running(ability) | Self::Asked(ability) => ability,
        }
    }

    /// The granted ability, if it's running here.
    #[must_use]
    pub fn running(self) -> Option<Entity> {
        match self {
            Self::Running(ability) => Some(ability),
            Self::Asked(_) => None,
        }
    }
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
