//! An effect starting and stopping counting: its tags granted, its modifiers on
//! its target.

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::{ModifierOf, Modifiers};
use bevy_mettle_core::tag::OwnedTags;

use super::Check;
use super::execution::modifiers;
use crate::EffectRegistry;
use crate::active::{ActiveEffect, EffectOf};

/// Whether `effect`'s ongoing requirement holds.
pub(super) fn holds_ongoing(world: &World, effect: Entity) -> bool {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return false;
    };
    let Some(ongoing) = &active.def.ongoing else {
        return true;
    };
    world.get::<OwnedTags>(of.0).is_some_and(|owner| {
        ongoing.holds(&Check {
            tags: &active.def.tags,
            owner: owner.counts(),
            source: &active.source_tags,
        })
    })
}

/// Makes `effect` count: its tags granted, its modifiers on its target (unless
/// it's periodic: it executes instead).
pub(crate) fn activate(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let (Some(active), Some(of)) = (
        world.get::<ActiveEffect>(effect),
        world.get::<EffectOf>(effect),
    ) else {
        return;
    };
    let target = of.0;
    let grants = active.def.grants.clone();
    let cues = active.def.cues.clone();
    let periodic = active.def.period.is_some();
    let modifiers = (!periodic).then(|| modifiers(world, registry, effect));
    if let Some(mut owned) = world.get_mut::<OwnedTags>(target) {
        owned.add_all(&grants);
    }
    for cue in cues.written() {
        crate::cue::add(world, target, cue, false);
    }
    if let Some(mut active) = world.get_mut::<ActiveEffect>(effect) {
        active.inhibited = false;
        active.granted = grants;
        active.cues = cues;
    }
    if let Some(modifiers) = modifiers {
        world
            .entity_mut(effect)
            .insert((Modifiers(modifiers), ModifierOf(target)));
    }
}

/// Makes `effect` stop counting: its tags taken back, its modifiers off its
/// target.
pub(crate) fn deactivate(world: &mut World, effect: Entity) {
    let Some(target) = world.get::<EffectOf>(effect).map(|of| of.0) else {
        return;
    };
    let Some(mut active) = world.get_mut::<ActiveEffect>(effect) else {
        return;
    };
    active.inhibited = true;
    let granted = std::mem::take(&mut active.granted);
    let cues = std::mem::take(&mut active.cues);
    if let Some(mut owned) = world.get_mut::<OwnedTags>(target) {
        owned.remove_all(&granted);
    }
    for cue in cues.written() {
        crate::cue::remove(world, target, cue, false);
    }
    world.entity_mut(effect).remove::<(Modifiers, ModifierOf)>();
}

/// Resolves `effect`'s modifiers again, if they count: as its stacks change.
pub(super) fn refresh(world: &mut World, registry: &EffectRegistry, effect: Entity) {
    let counts = world
        .get::<ActiveEffect>(effect)
        .is_some_and(|active| !active.inhibited && active.def.period.is_none());
    if counts {
        let modifiers = modifiers(world, registry, effect);
        world.entity_mut(effect).insert(Modifiers(modifiers));
    }
}
