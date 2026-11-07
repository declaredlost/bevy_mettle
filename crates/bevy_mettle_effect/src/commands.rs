//! Applying and removing effects: as commands, or on the world.

use bevy::prelude::*;
use bevy_mettle_core::tag::Tags;

use crate::{EffectSpec, engine};

/// Applying and removing effects on an entity, as commands.
pub trait EffectCommands {
    /// Applies `spec` to this actor.
    fn apply_effect(&mut self, spec: EffectSpec) -> &mut Self;

    /// Takes every effect tagged any of `tags` off this actor.
    fn remove_effects(&mut self, tags: Tags) -> &mut Self;

    /// Takes every effect applied under `activation` off this actor: what a
    /// rejected prediction does.
    fn remove_activation(&mut self, activation: u64) -> &mut Self;

    /// Takes this active effect off its actor.
    fn remove_effect(&mut self) -> &mut Self;
}

impl EffectCommands for EntityCommands<'_> {
    fn apply_effect(&mut self, spec: EffectSpec) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let target = entity.id();
            engine::apply(entity.into_world_mut(), target, spec);
        })
    }

    fn remove_effects(&mut self, tags: Tags) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let target = entity.id();
            engine::remove_tagged(entity.into_world_mut(), target, &tags, 0);
        })
    }

    fn remove_activation(&mut self, activation: u64) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let target = entity.id();
            engine::remove_activation(entity.into_world_mut(), target, activation);
        })
    }

    fn remove_effect(&mut self) -> &mut Self {
        self.queue(|entity: EntityWorldMut| {
            let effect = entity.id();
            engine::remove(entity.into_world_mut(), effect, false, 0);
        })
    }
}

/// Applying and removing effects on the world, at once.
///
/// In an exclusive system, or a test.
pub trait EffectWorld {
    /// Applies `spec` to `target`.
    fn apply_effect(&mut self, target: Entity, spec: EffectSpec);

    /// Takes every effect tagged any of `tags` off `target`.
    fn remove_effects(&mut self, target: Entity, tags: &Tags);

    /// Takes the active effect `effect` off its actor.
    fn remove_effect(&mut self, effect: Entity);

    /// The server confirmed `activation`: what was predicted under it stays,
    /// until the server's own effects replace it; its base changes, and the
    /// stacks it added to the server's effects, give way to the server's.
    ///
    /// So confirm it no earlier than the server's state as of its answer has
    /// arrived, or what it paid shows unpaid until it does. Nothing more is
    /// predicted under it.
    fn confirm_prediction(&mut self, activation: u64);

    /// The server rejected `activation`: every effect, stack and base change
    /// predicted under it undone. Nothing more is predicted under it.
    fn reject_prediction(&mut self, activation: u64);

    /// What applying `spec` to `target` as an instant effect would change,
    /// without changing it; `None` if there's no such effect.
    fn preview_effect(
        &self,
        target: Entity,
        spec: &EffectSpec,
    ) -> Option<Vec<crate::code::BaseChange>>;
}

impl EffectWorld for World {
    fn apply_effect(&mut self, target: Entity, spec: EffectSpec) {
        engine::apply(self, target, spec);
    }

    fn remove_effects(&mut self, target: Entity, tags: &Tags) {
        engine::remove_tagged(self, target, tags, 0);
    }

    fn remove_effect(&mut self, effect: Entity) {
        engine::remove(self, effect, false, 0);
    }

    fn confirm_prediction(&mut self, activation: u64) {
        crate::net::confirm(self, activation);
    }

    fn reject_prediction(&mut self, activation: u64) {
        crate::net::rollback(self, activation);
    }

    fn preview_effect(
        &self,
        target: Entity,
        spec: &EffectSpec,
    ) -> Option<Vec<crate::code::BaseChange>> {
        engine::preview(self, target, spec)
    }
}
