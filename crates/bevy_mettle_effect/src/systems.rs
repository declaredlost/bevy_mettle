//! What runs each tick.

use bevy::prelude::*;

use crate::{ActiveEffect, EffectRegistry, engine};

/// Gives active effects their new definitions, as the registry's
/// replaced.
pub(crate) fn relink(world: &mut World, effects: &mut QueryState<Entity, With<ActiveEffect>>) {
    let registry = world.resource::<EffectRegistry>().clone();
    let all: Vec<Entity> = effects.iter(world).collect();
    for effect in all {
        engine::relink(world, &registry, effect);
    }
}

/// Moves every active effect on by the tick's time.
pub(crate) fn tick(world: &mut World, effects: &mut QueryState<Entity, With<ActiveEffect>>) {
    let seconds = world.resource::<Time>().delta_secs();
    let registry = world.resource::<EffectRegistry>().clone();
    let all: Vec<Entity> = effects.iter(world).collect();
    for effect in all {
        engine::step(world, &registry, effect, seconds);
    }
}
