//! Effects on their targets: each an entity of its own, related to its target.

use std::sync::Arc;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::{OwnedTags, TagCounts, Tags};

use crate::data::Effect;
use crate::net::Proxy;
use crate::{EffectSpec, engine};

/// An effect on its target: an entity of its own, [`EffectOf`] the target.
///
/// Unreal's active gameplay effect. While it counts, it carries its modifiers
/// as [`Modifiers`](bevy_mettle_attribute::modifier::Modifiers)
/// [`ModifierOf`](bevy_mettle_attribute::modifier::ModifierOf) the target, and
/// the target owns the tags it grants; while it's inhibited, neither, and its
/// time stands still.
#[derive(Component, Debug)]
#[component(on_discard = take_back)]
pub struct ActiveEffect {
    pub(crate) spec: EffectSpec,
    pub(crate) def: Arc<Effect>,
    pub(crate) stacks: u32,
    /// How long it lasts in full, if not forever.
    pub(crate) duration: Option<f32>,
    pub(crate) remaining: Option<f32>,
    /// How often it executes, if it does.
    pub(crate) period: Option<f32>,
    pub(crate) until_period: f32,
    pub(crate) inhibited: bool,
    /// The tags it granted, to take back.
    pub(crate) granted: Tags,
    /// What its source owned as it applied.
    pub(crate) source_tags: TagCounts,
    /// Of its stacks, those a client predicted onto the server's effect.
    pub(crate) predicted: u32,
    /// Its modifiers' attribute magnitudes, as captured as it applied.
    pub(crate) captured: Vec<Option<f32>>,
    /// Whether mettle is taking it off (removed, or a prediction undone), so
    /// it's told there, or not at all.
    pub(crate) removing: bool,
}

impl ActiveEffect {
    /// What was applied.
    #[must_use]
    pub fn spec(&self) -> &EffectSpec {
        &self.spec
    }

    /// Its definition.
    #[must_use]
    pub fn def(&self) -> &Effect {
        &self.def
    }

    /// Its effect's id.
    #[must_use]
    pub fn id(&self) -> &Id<Effect> {
        &self.def.id
    }

    /// How many stacks of it there are.
    #[must_use]
    pub fn stacks(&self) -> u32 {
        self.stacks
    }

    /// The seconds it has left, if it doesn't last forever.
    #[must_use]
    pub fn remaining(&self) -> Option<f32> {
        self.remaining
    }

    /// Whether it's inhibited: its ongoing requirement doesn't hold.
    #[must_use]
    pub fn is_inhibited(&self) -> bool {
        self.inhibited
    }
}

/// Takes back the tags `ActiveEffect` granted, as it's taken off its entity
/// however: removed, despawned with its target, or replicated away.
///
/// Taken off by the game (despawned, or the component removed) rather than by
/// mettle, it's told and chained as `remove_effect` would: once commands are
/// applied, if its target's still there. A client's shown effect, the
/// server's, goes as the server says, with nothing told.
fn take_back(mut world: DeferredWorld, context: HookContext) {
    let Some(target) = world.get::<EffectOf>(context.entity).map(|of| of.0) else {
        return;
    };
    let Some(active) = world.get::<ActiveEffect>(context.entity) else {
        return;
    };
    let granted = active.granted.clone();
    let unasked = (!active.removing && world.get::<Proxy>(context.entity).is_none())
        .then(|| (active.def.clone(), active.spec.clone()));
    if !granted.is_empty()
        && let Some(mut owned) = world.get_mut::<OwnedTags>(target)
    {
        owned.remove_all(&granted);
    }
    if let Some((def, spec)) = unasked {
        let effect = context.entity;
        world.commands().queue(move |world: &mut World| {
            engine::removed_unasked(world, target, effect, &def, &spec);
        });
    }
}

/// On an active effect: the actor it's on, as a Bevy relationship.
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
#[relationship(relationship_target = ActiveEffects)]
pub struct EffectOf(pub Entity);

/// On an actor: the effects on it, kept by [`EffectOf`].
///
/// Despawning the actor despawns them.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = EffectOf, linked_spawn)]
pub struct ActiveEffects(Vec<Entity>);

impl ActiveEffects {
    /// The effects, in the order they were applied.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}
