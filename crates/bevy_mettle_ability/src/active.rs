//! Abilities granted to actors, and those active.

use std::sync::Arc;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy_mettle_core::tag::{OwnedTags, Tags};

use crate::AbilityId;
use crate::behaviour::BehaviourId;
use crate::data::AbilityDef;
use crate::events::GameplayEvent;

/// An ability granted to an actor: an entity of its own, [`AbilityOf`] it.
///
/// Unreal's `GameplayAbilitySpec`. While it's active, it carries
/// [`ActiveAbility`] and its behaviour's component. One an effect granted is
/// [`GrantedBy`] it, and taken away with it however it goes. Despawned while
/// it's active, it takes back the tags it granted its owner.
#[derive(Component, Debug)]
#[component(on_discard = take_back)]
pub struct GrantedAbility {
    pub(crate) def: Arc<AbilityDef>,
    pub(crate) level: f32,
    /// The effect that granted it, if one did: as its [`GrantedBy`] says.
    pub(crate) source: Option<Entity>,
    /// The tags it granted its owner as it activated, to take back.
    pub(crate) granted: Tags,
    /// The cues it held on its owner as it activated, to let go of.
    pub(crate) cues: Tags,
    /// The behaviour whose component it activated with, to take off: its
    /// definition may have been replaced since.
    pub(crate) running: Option<BehaviourId>,
    /// Whether this activation paid its cost.
    pub(crate) cost_paid: bool,
    /// Whether this activation started its cooldown.
    pub(crate) cooldown_started: bool,
}

impl GrantedAbility {
    pub(crate) fn new(def: Arc<AbilityDef>, level: f32, source: Option<Entity>) -> Self {
        Self {
            def,
            level,
            source,
            granted: Tags::new(),
            cues: Tags::new(),
            running: None,
            cost_paid: false,
            cooldown_started: false,
        }
    }

    /// Its definition.
    #[must_use]
    pub fn def(&self) -> &AbilityDef {
        &self.def
    }

    /// Its ability's id.
    #[must_use]
    pub fn id(&self) -> AbilityId {
        self.def.id
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

    /// Whether this activation's cost is paid: once it's committed it, or
    /// its cost alone. One without a cost has nothing to pay.
    #[must_use]
    pub fn is_cost_paid(&self) -> bool {
        self.cost_paid || self.def.cost.is_none()
    }

    /// Whether this activation's cooldown is started: once it's committed it,
    /// or its cooldown alone. One without a cooldown has nothing to start.
    #[must_use]
    pub fn is_cooldown_started(&self) -> bool {
        self.cooldown_started || self.def.cooldown.is_none()
    }

    /// Whether this activation's committed in full: everything it has of a
    /// cost and a cooldown, paid and started.
    #[must_use]
    pub fn is_committed(&self) -> bool {
        self.is_cost_paid() && self.is_cooldown_started()
    }
}

/// Takes back the tags `GrantedAbility` granted its owner as it activated, as
/// it's taken off its entity however: despawned, or replicated away.
fn take_back(mut world: DeferredWorld, context: HookContext) {
    let Some(granted) = world.get::<GrantedAbility>(context.entity) else {
        return;
    };
    if granted.granted.is_empty() {
        return;
    }
    let granted = granted.granted.clone();
    let Some(actor) = world.get::<AbilityOf>(context.entity).map(|of| of.0) else {
        return;
    };
    if let Some(mut owned) = world.get_mut::<OwnedTags>(actor) {
        owned.remove_all(&granted);
    }
}

/// On a granted ability: the effect that granted it, as a Bevy relationship.
///
/// The effect going, however it goes, takes the ability away with it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[relationship(relationship_target = GrantedAbilities)]
pub struct GrantedBy(pub Entity);

/// On an active effect: the abilities it granted, kept by [`GrantedBy`].
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = GrantedBy)]
pub struct GrantedAbilities(Vec<Entity>);

impl GrantedAbilities {
    /// The abilities, in the order they were granted.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}

/// On a granted ability: that it's active, and how; its behaviour reads it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ActiveAbility {
    /// Who it's active for.
    pub actor: Entity,
    /// This activation: what its cost and cooldown are applied under, and what
    /// a rejected prediction takes back.
    pub activation: u64,
    /// Its level.
    pub level: f32,
    /// The gameplay event that activated it, if one did.
    pub event: Option<GameplayEvent>,
}

/// On a granted ability: the actor it's granted to, as a Bevy relationship.
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

/// On an actor: the abilities granted to it, kept by [`AbilityOf`].
///
/// Despawning the actor despawns them.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = AbilityOf, linked_spawn)]
pub struct Abilities(Vec<Entity>);

impl Abilities {
    /// The abilities, in the order they were granted.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}
