//! What time brings an active effect each tick, done with the world.

use bevy::prelude::*;
use bevy_mettle_core::tag::OwnedTags;

use super::Check;
use super::count::{activate, deactivate, holds_ongoing};
use super::execution::execute;
use super::remove::{expire, remove};
use crate::EffectRegistry;
use crate::active::{ActiveEffect, EffectOf};

/// What's due of an active effect this tick, as time moved it on: for the
/// world to weigh and do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Due {
    /// How many times its period came round.
    pub(crate) executions: u32,
}

/// Does what's due of `effect` this tick, weighed as it stands now, after
/// what's been done before it: taken off if its removal requirement holds;
/// counting as its ongoing one says; its periods executed while it counts;
/// run out if its time is up now (another effect may have refreshed it).
pub(crate) fn settle(world: &mut World, registry: &EffectRegistry, effect: Entity, due: Due) {
    if removes(world, effect) {
        remove(world, effect, false, 0);
        return;
    }
    let Some(inhibited) = world
        .get::<ActiveEffect>(effect)
        .map(|active| active.inhibited)
    else {
        return;
    };
    match (inhibited, holds_ongoing(world, effect)) {
        (true, true) => activate(world, registry, effect),
        (false, false) => {
            deactivate(world, effect);
            return;
        }
        (true, false) => return,
        (false, true) => {}
    }
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let (spec, def, stacks, source_tags, captured) = (
        active.spec.clone(),
        active.def.clone(),
        active.stacks,
        active.source_tags.clone(),
        active.captured.clone(),
    );
    for _ in 0..due.executions {
        execute(
            world,
            registry,
            target,
            &spec,
            &def,
            stacks,
            &source_tags,
            &captured,
        );
    }
    let expired = world
        .get::<ActiveEffect>(effect)
        .and_then(|active| active.remaining)
        .is_some_and(|remaining| remaining <= 0.0);
    if expired {
        expire(world, registry, effect, &def);
    }
}

/// Whether `effect`'s removal requirement holds now.
fn removes(world: &World, effect: Entity) -> bool {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return false;
    };
    let (Some(end_if), Some(owner)) = (&active.def.end_if, world.get::<OwnedTags>(of.0)) else {
        return false;
    };
    end_if.holds(&Check {
        tags: &active.def.tags,
        owner: owner.counts(),
        source: &active.source_tags,
    })
}
