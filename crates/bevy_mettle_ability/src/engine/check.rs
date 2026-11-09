//! Whether an ability could activate, could pay, and how long its cooldown has
//! left: asked with nothing done.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld};
use bevy_mettle_core::tag::{OwnedTags, Tags};
use bevy_mettle_effect::active::{ActiveEffect, ActiveEffects};
use bevy_mettle_effect::code::Changes;
use bevy_mettle_effect::{EffectRegistry, EffectSpec, EffectWorld};

use super::{Check, abilities_of, interrupts};
use crate::active::{AbilityOf, ActiveAbility, GrantedAbility};
use crate::events::Failure;

/// Whether `ability` can activate now, and if not, why.
pub(crate) fn can_activate(world: &World, ability: Entity) -> Result<(), Failure> {
    let (Some(granted), Some(of)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<AbilityOf>(ability),
    ) else {
        return Err(Failure::NotGranted);
    };
    let actor = of.0;
    if world.get::<ActiveAbility>(ability).is_some() {
        return Err(Failure::Active);
    }
    let Some(owner) = world.get::<OwnedTags>(actor) else {
        return Err(Failure::NotGranted);
    };
    let def = &granted.def;
    if cooldown_grants(world, granted).is_some_and(|cooling| owner.contains_any(cooling)) {
        return Err(Failure::Cooldown);
    }
    if !affords(world, actor, granted) {
        return Err(Failure::Cost);
    }
    let blocked_by_owner = def.blocked_by.written().any(|tag| owner.contains(tag));
    let blocked_by_active = abilities_of(world, actor).into_iter().any(|other| {
        other != ability
            && world.get::<ActiveAbility>(other).is_some()
            && world
                .get::<GrantedAbility>(other)
                .is_some_and(|other| def.tags.contains_any(&other.def.blocks))
    });
    if blocked_by_owner || blocked_by_active {
        return Err(Failure::Blocked);
    }
    if def
        .require
        .as_ref()
        .is_some_and(|require| !require.holds(&Check::of(granted, owner)))
    {
        return Err(Failure::Requirement);
    }
    if interrupts(granted, owner) {
        return Err(Failure::Interrupted);
    }
    Ok(())
}

/// What `granted`'s cooldown grants: while its owner owns any of it, it's
/// cooling down.
fn cooldown_grants<'w>(world: &'w World, granted: &GrantedAbility) -> Option<&'w Tags> {
    world
        .get_resource::<EffectRegistry>()?
        .get(granted.def.cooldown?)
        .map(|effect| &effect.grants)
}

/// Whether the granted `ability`'s owner could pay its cost now.
pub(crate) fn can_afford(world: &World, ability: Entity) -> bool {
    let (Some(granted), Some(of)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<AbilityOf>(ability),
    ) else {
        return false;
    };
    affords(world, of.0, granted)
}

/// The seconds left on the granted `ability`'s cooldown, if it's cooling down
/// (as [`can_activate`] weighs it): the longest of its owner's counting
/// effects granting what its cooldown grants. Cooling down with no such effect,
/// or one lasting until taken off, it's forever.
pub(crate) fn cooldown_remaining(world: &World, ability: Entity) -> Option<f32> {
    let (Some(granted), Some(of)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<AbilityOf>(ability),
    ) else {
        return None;
    };
    let cooling = cooldown_grants(world, granted)?;
    if !world
        .get::<OwnedTags>(of.0)
        .is_some_and(|owner| owner.contains_any(cooling))
    {
        return None;
    }
    let longest = world
        .get::<ActiveEffects>(of.0)
        .into_iter()
        .flat_map(ActiveEffects::iter)
        .filter_map(|effect| world.get::<ActiveEffect>(effect))
        .filter(|effect| !effect.is_inhibited() && effect.def().grants.contains_any(cooling))
        .map(|effect| effect.remaining().unwrap_or(f32::INFINITY))
        .reduce(f32::max);
    Some(longest.unwrap_or(f32::INFINITY))
}

/// Whether `actor` can pay `granted`'s cost: applying it, each change after
/// the one before, wouldn't leave an attribute below 0.
pub(super) fn affords(world: &World, actor: Entity, granted: &GrantedAbility) -> bool {
    let Some(cost) = granted.def.cost else {
        return true;
    };
    let spec = EffectSpec::from_id(cost).level(granted.level).source(actor);
    let Some(changes) = world.preview_effect(actor, &spec) else {
        return true;
    };
    let mut after = HashMap::new();
    for change in &changes {
        // Not worked out yet (spawned this frame), it's its base, as its
        // `Current` is.
        // Routes are routed in a preview already: what's left changes
        // attributes.
        let Changes::Attribute(attribute) = change.changes else {
            continue;
        };
        let now = after.get(&attribute).copied().unwrap_or_else(|| {
            world
                .value_at(actor, attribute, &AttributeContext::new())
                .or_else(|| world.base_at(actor, attribute))
                .unwrap_or_default()
        });
        after.insert(attribute, change.op.apply(now, change.value));
    }
    after.values().all(|value| *value >= 0.0)
}
