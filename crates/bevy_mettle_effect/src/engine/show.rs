//! Effects given new definitions, and a client's shown from the server's
//! replicas.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::Provisional;
use bevy_mettle_core::tag::{TagRegistry, Tags};

use super::apply::capture;
use super::count::{activate, deactivate, refresh};
use super::effects_on;
use super::remove::{remove, remove_quietly};
use crate::EffectRegistry;
use crate::active::{ActiveEffect, EffectOf};
use crate::context::ContextDataRegistry;
use crate::net::{self, EffectReplica};

/// Gives every active effect its definition from `registry`, as it's replaced:
/// one whose effect is gone is taken off.
pub(crate) fn relink(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let Some(active) = world.get::<ActiveEffect>(effect) else {
        return;
    };
    let Some(def) = registry.get(active.def.id).cloned() else {
        remove(world, effect, false, 0);
        return;
    };
    if Arc::ptr_eq(&def, &active.def) {
        return;
    }
    let counted = !active.inhibited;
    if counted {
        deactivate(world, effect);
    }
    let captured = world
        .get::<ActiveEffect>(effect)
        .zip(world.get::<EffectOf>(effect))
        .map(|(active, of)| capture(world, of.0, &active.spec, &def))
        .unwrap_or_default();
    if let Some(mut active) = world.get_mut::<ActiveEffect>(effect) {
        active.def = def;
        active.captured = captured;
    }
    super::live::watch(world, effect);
    if counted {
        activate(world, registry, effect);
    }
}

/// Takes the stack a prediction added off `effect`. The server's effect, shown
/// here, keeps the stacks the server says it has.
pub(crate) fn unstack(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let proxy = world.get::<net::Proxy>(effect).is_some();
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    if proxy {
        if active.predicted > 0 {
            active.predicted -= 1;
            active.stacks = active.stacks.saturating_sub(1).max(1);
            if active.predicted == 0 {
                world.entity_mut(effect).remove::<Provisional>();
            }
            refresh(world, registry, effect);
        }
    } else if active.stacks > 1 {
        active.stacks -= 1;
        refresh(world, registry, effect);
    } else {
        remove_quietly(world, effect);
    }
}

/// Shows, on a client, the server's effect `entity` on `target` from its
/// replica: counting as it does there, but not ticked here. A predicted effect
/// it stands for (the same effect, from the same source, under the same
/// activation) is taken off.
pub(crate) fn show(
    world: &mut World,
    registry: &EffectRegistry,
    tags: &TagRegistry,
    entity: Entity,
    target: Entity,
    replica: &EffectReplica,
) {
    let data = world
        .get_resource::<ContextDataRegistry>()
        .cloned()
        .unwrap_or_default();
    let Some(def) = registry.get(replica.effect).cloned() else {
        warn!("there's no effect {:?} to show", replica.effect);
        return;
    };
    if let Some(activation) = replica.activation {
        let predicted: Vec<Entity> = effects_on(world, target)
            .into_iter()
            .filter(|other| {
                *other != entity
                    && world.get::<net::PredictedUnder>(*other)
                        == Some(&net::PredictedUnder(activation))
                    && world.get::<ActiveEffect>(*other).is_some_and(|active| {
                        active.def.id == def.id
                            && active.spec.context.source == replica.context.source
                    })
            })
            .collect();
        for other in predicted {
            remove_quietly(world, other);
        }
    }
    world.entity_mut(entity).insert((
        ActiveEffect {
            spec: replica.spec(tags, &data),
            def,
            stacks: replica.stacks,
            duration: replica.duration,
            remaining: replica.remaining,
            period: None,
            until_period: 0.0,
            inhibited: true,
            owner_tags: Tags::new(),
            cues: Tags::new(),
            source_tags: replica.source_tags(tags),
            predicted: 0,
            captured: replica.captured.clone(),
            removing: false,
        },
        net::Proxy,
    ));
    super::live::watch(world, entity);
    if !replica.inhibited {
        activate(world, registry, entity);
    }
}

/// Keeps the shown effect `entity` as its replica now says, with the stacks
/// predicted onto it on top.
pub(crate) fn update_shown(
    world: &mut World,
    registry: &EffectRegistry,
    entity: Entity,
    replica: &EffectReplica,
) {
    let Some(mut active) = world.get_mut::<ActiveEffect>(entity) else {
        return;
    };
    let stacks = replica.stacks + active.predicted;
    let restack = active.stacks != stacks;
    let was_inhibited = active.inhibited;
    active.stacks = stacks;
    active.duration = replica.duration;
    active.remaining = replica.remaining;
    match (was_inhibited, replica.inhibited) {
        (true, false) => activate(world, registry, entity),
        (false, true) => deactivate(world, entity),
        _ if restack => refresh(world, registry, entity),
        _ => {}
    }
}
