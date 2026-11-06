//! Abilities granted to actors, and those active.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::Tags;

use crate::Ability;
use crate::events::GameplayEvent;

/// An ability granted to an actor (Unreal's `GameplayAbilitySpec`): an
/// entity of its own, [`AbilityOf`] the actor. While it's active, it
/// carries [`ActiveAbility`] and its behaviour's component.
#[derive(Component, Debug)]
pub struct GrantedAbility {
    pub(crate) def: Arc<Ability>,
    pub(crate) level: f32,
    /// The effect that granted it, if one did: taken away with it.
    pub(crate) source: Option<Entity>,
    /// The tags it granted its owner as it activated, to take back.
    pub(crate) granted: Tags,
    pub(crate) committed: bool,
}

impl GrantedAbility {
    pub(crate) fn new(def: Arc<Ability>, level: f32, source: Option<Entity>) -> Self {
        Self {
            def,
            level,
            source,
            granted: Tags::new(),
            committed: false,
        }
    }

    /// Its definition.
    #[must_use]
    pub fn def(&self) -> &Ability {
        &self.def
    }

    /// Its ability's id.
    #[must_use]
    pub fn id(&self) -> &Id<Ability> {
        &self.def.id
    }

    /// Its level: what its cost's and cooldown's curves are read at.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.level
    }

    /// The effect that granted it, if one did.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.source
    }

    /// Whether its cost and cooldown are applied, this activation.
    #[must_use]
    pub fn is_committed(&self) -> bool {
        self.committed
    }
}

/// On a granted ability: that it's active, and how. Its behaviour's
/// systems read it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActiveAbility {
    /// Who it's active for.
    pub actor: Entity,
    /// This activation: what its cost and cooldown are applied under, and
    /// what a rejected prediction takes back.
    pub activation: u64,
    /// Its level.
    pub level: f32,
    /// The gameplay event that activated it, if one did.
    pub event: Option<GameplayEvent>,
}

/// On a granted ability: its actor.
#[derive(
    Component,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize
)]
#[relationship(relationship_target = Abilities)]
pub struct AbilityOf(pub Entity);

/// On an actor: the abilities granted to it. Kept by [`AbilityOf`];
/// despawning the actor despawns them.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = AbilityOf, linked_spawn)]
pub struct Abilities(Vec<Entity>);

impl Abilities {
    /// The abilities, in the order they were granted.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}
