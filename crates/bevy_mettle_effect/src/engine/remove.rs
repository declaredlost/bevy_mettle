//! Taking effects off: removed, by tag, by activation, run out, quietly, or by
//! the game.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::tag::Tags;

use super::apply::chain;
use super::count::{deactivate, refresh};
use super::effects_on;
use crate::active::{ActiveEffect, EffectOf};
use crate::data::{EffectDef, Expire};
use crate::events::{EffectRemoved, EffectStacked};
use crate::{EffectRegistry, EffectSpec};

/// Takes `effect` off its target: run out of time if `expired`.
pub(crate) fn remove(world: &mut World, effect: Entity, expired: bool, depth: u32) {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let def = active.def.clone();
    let spec = active.spec.clone();
    if !active.inhibited {
        deactivate(world, effect);
    }
    mark_removing(world, effect);
    world.despawn(effect);
    world.trigger(EffectRemoved {
        entity: target,
        effect,
        id: def.id,
        expired,
    });
    chain(world, target, &spec, &def.on_remove, depth);
    if expired {
        chain(world, target, &spec, &def.on_expire, depth);
    }
}

/// Takes every effect tagged any of `tags` off `target`.
pub(crate) fn remove_tagged(world: &mut World, target: Entity, tags: &Tags, depth: u32) {
    let tagged: Vec<Entity> = effects_on(world, target)
        .into_iter()
        .filter(|effect| {
            world
                .get::<ActiveEffect>(*effect)
                .is_some_and(|active| active.def.tags.contains_any(tags))
        })
        .collect();
    for effect in tagged {
        remove(world, effect, false, depth);
    }
}

/// Takes every effect applied under `activation` off `target`.
pub(crate) fn remove_activation(world: &mut World, target: Entity, activation: Activation) {
    let applied: Vec<Entity> = effects_on(world, target)
        .into_iter()
        .filter(|effect| {
            world
                .get::<ActiveEffect>(*effect)
                .is_some_and(|active| active.spec.activation == Some(activation))
        })
        .collect();
    for effect in applied {
        remove(world, effect, false, 0);
    }
}

/// `effect` has run out of time: as its stacking says.
pub(super) fn expire(
    world: &mut World,
    registry: &EffectRegistry,
    effect: Entity,
    def: &Arc<EffectDef>,
) {
    let policy = def.stacking.as_ref().map(|stacking| stacking.expire);
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    match policy {
        Some(Expire::Refresh) => active.remaining = active.duration,
        Some(Expire::RemoveOne) if active.stacks > 1 => {
            active.stacks -= 1;
            active.remaining = active.duration;
            let stacks = active.stacks;
            let target = world.get::<EffectOf>(effect).map(|of| of.0);
            refresh(world, registry, effect);
            if let Some(target) = target {
                world.trigger(EffectStacked {
                    entity: target,
                    effect,
                    id: def.id,
                    stacks,
                });
            }
        }
        _ => remove(world, effect, true, 0),
    }
}

/// Takes `effect` off its target with nothing told and nothing chained: a
/// prediction undone, or replaced by the server's own.
pub(crate) fn remove_quietly(world: &mut World, effect: Entity) {
    let Some(active) = world.get::<ActiveEffect>(effect) else {
        return;
    };
    if !active.inhibited {
        deactivate(world, effect);
    }
    mark_removing(world, effect);
    world.despawn(effect);
}

/// Marks `effect` as being taken off by mettle, which tells of it itself.
pub(super) fn mark_removing(world: &mut World, effect: Entity) {
    if let Some(mut active) = world.get_mut::<ActiveEffect>(effect) {
        active.removing = true;
    }
}

/// `effect`, taken off `target` by the game rather than through
/// `remove_effect`: told and chained as removed, if `target`'s still there.
pub(crate) fn removed_unasked(
    world: &mut World,
    target: Entity,
    effect: Entity,
    def: &EffectDef,
    spec: &EffectSpec,
) {
    if world.get_entity(target).is_err() {
        return;
    }
    world.trigger(EffectRemoved {
        entity: target,
        effect,
        id: def.id,
        expired: false,
    });
    chain(world, target, spec, &def.on_remove, 0);
}
