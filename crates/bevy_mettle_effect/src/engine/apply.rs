//! Applying an effect: checked, blocked, stacked or spawned, and chained.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::modifier::Provisional;
use bevy_mettle_core::registry::EffectId;
use bevy_mettle_core::tag::{OwnedTags, TagCounts, Tags};

use super::count::{activate, holds_ongoing, refresh};
use super::execution::{execute, read, resolve};
use super::remove::remove_tagged;
use super::{Check, DEPTH, effects_on, source_tags};
use crate::active::{ActiveEffect, EffectOf};
use crate::code::ExecutionContext;
use crate::data::{EffectDef, EffectDuration, Reapply, StackBy, Stacking};
use crate::events::{Blocked, EffectApplied, EffectBlocked, EffectStacked};
use crate::{EffectRegistry, EffectSpec, net};

/// Applies `spec` to `target`, its effect resolved to its number.
pub(crate) fn apply(world: &mut World, target: Entity, spec: &EffectSpec) {
    let Some(registry) = world.get_resource::<EffectRegistry>() else {
        warn!(
            "effect `{}` isn't applied: there's no effect registry",
            spec.label()
        );
        return;
    };
    let Some(effect) = spec.effect_id(registry) else {
        warn!(
            "effect `{}` isn't applied: it isn't added, or content hasn't it",
            spec.label()
        );
        return;
    };
    apply_at(world, target, effect, spec.clone(), 0);
}

/// Applies `effect` as `spec` says to `target`, `depth` effects deep in a
/// chain.
pub(super) fn apply_at(
    world: &mut World,
    target: Entity,
    effect: EffectId,
    spec: EffectSpec,
    depth: u32,
) {
    let Some(registry) = world.get_resource::<EffectRegistry>().cloned() else {
        warn!("effect {effect:?} isn't applied: there's no effect registry");
        return;
    };
    let Some(def) = registry.get(effect).cloned() else {
        warn!("there's no effect {effect:?}: content reloaded without it");
        return;
    };
    if depth > DEPTH {
        warn!(
            "effect `{}` applies effects more than {DEPTH} deep: taken for a loop, and stopped",
            def.name
        );
        return;
    }
    if net::answered(world, &spec) {
        return;
    }
    let source_tags = source_tags(world, spec.context.source);
    if world.get::<OwnedTags>(target).is_none() {
        warn!(
            "effect `{}` isn't applied to {target}: it owns no tags",
            def.name
        );
        return;
    }
    if let Some(reason) = blocked(world, target, &def, &source_tags) {
        world.trigger(EffectBlocked {
            entity: target,
            id: def.id,
            reason,
        });
        return;
    }
    if !def.removes.is_empty() {
        remove_tagged(world, target, &def.removes, depth);
    }
    if !def.lasts() {
        let captured = capture(world, target, &spec, &def);
        execute(
            world,
            &registry,
            target,
            &spec,
            &def,
            1,
            &source_tags,
            &captured,
        );
        chain(world, target, &spec, &def.on_apply, depth);
        return;
    }
    if let Some(stacking) = &def.stacking
        && let Some(existing) = stack_of(world, target, def.id, stacking.by, spec.context.source)
    {
        stack(world, &registry, existing, stacking, &spec, depth);
        return;
    }
    let Some(effect) = spawn(world, &registry, target, &spec, &def, source_tags) else {
        return;
    };
    if let Some(activation) = net::predicting(world, &spec) {
        net::spawned(world, activation, effect);
    }
    world.trigger(EffectApplied {
        entity: target,
        effect,
        id: def.id,
    });
    chain(world, target, &spec, &def.on_apply, depth);
}

/// Why `def` can't apply to `target`, if it can't.
pub(super) fn blocked(
    world: &World,
    target: Entity,
    def: &EffectDef,
    source_tags: &TagCounts,
) -> Option<Blocked> {
    let owner = world.get::<OwnedTags>(target)?;
    let check = Check {
        tags: &def.tags,
        owner: owner.counts(),
        source: source_tags,
    };
    if def
        .require
        .as_ref()
        .is_some_and(|require| !require.holds(&check))
    {
        Some(Blocked::Requirement)
    } else if immune(world, target, &def.tags) {
        Some(Blocked::Immunity)
    } else {
        None
    }
}

/// Spawns `def`, applied as `spec`, on `target`: counting at once if its
/// ongoing requirement holds, and executing at once if it says so. Nothing, if
/// the duration it comes to can't be: not a number, or none at all.
pub(super) fn spawn(
    world: &mut World,
    registry: &EffectRegistry,
    target: Entity,
    spec: &EffectSpec,
    def: &Arc<EffectDef>,
    source_tags: TagCounts,
) -> Option<Entity> {
    let captured = capture(world, target, spec, def);
    let (duration, period) = {
        let context = ExecutionContext {
            world: &*world,
            target,
            def,
            spec,
            stacks: 1,
            captured: &captured,
            reads: &[],
        };
        let duration = match &def.duration {
            EffectDuration::For(seconds) => Some(resolve(&context, registry, seconds)),
            EffectDuration::Instant | EffectDuration::Infinite => None,
        };
        if let Some(seconds) = duration
            && !(seconds.is_finite() && seconds > 0.0)
        {
            warn!(
                "effect `{}` would last {seconds} seconds: it isn't applied",
                def.name
            );
            return None;
        }
        let period = def
            .period
            .as_ref()
            .map(|period| resolve(&context, registry, &period.every))
            .filter(|every| {
                let fine = *every > 0.0;
                if !fine {
                    warn!(
                        "effect `{}` has a period of {every}: it executes on apply alone",
                        def.name
                    );
                }
                fine
            });
        (duration, period)
    };
    let effect = world
        .spawn((
            ActiveEffect {
                spec: spec.clone(),
                def: def.clone(),
                stacks: 1,
                duration,
                remaining: duration,
                period,
                until_period: period.unwrap_or_default(),
                inhibited: true,
                granted: Tags::new(),
                cues: Tags::new(),
                source_tags: source_tags.clone(),
                predicted: 0,
                captured: captured.clone(),
                removing: false,
            },
            EffectOf(target),
        ))
        .id();
    super::live::watch(world, effect);
    if holds_ongoing(world, effect) {
        activate(world, registry, effect);
        if def.period.as_ref().is_some_and(|period| period.on_apply) {
            execute(
                world,
                registry,
                target,
                spec,
                def,
                1,
                &source_tags,
                &captured,
            );
        }
    }
    Some(effect)
}

/// What `def` captures applied to `target` with `spec`, by slot: each
/// snapshot as `spec` took it, or as it is now; none for one read live.
pub(super) fn capture(
    world: &World,
    target: Entity,
    spec: &EffectSpec,
    def: &EffectDef,
) -> Vec<Option<f32>> {
    let context = ExecutionContext {
        world,
        target,
        def,
        spec,
        stacks: 1,
        captured: &[],
        reads: &[],
    };
    def.captures
        .iter()
        .map(|capture| {
            if !capture.snapshot {
                return None;
            }
            spec.snapshot_of(capture)
                .or_else(|| read(&context, capture))
                .or(Some(0.0))
        })
        .collect()
}

/// Whether an effect on `target` keeps effects tagged `tags` off it.
pub(super) fn immune(world: &World, target: Entity, tags: &Tags) -> bool {
    effects_on(world, target).into_iter().any(|effect| {
        world
            .get::<ActiveEffect>(effect)
            .is_some_and(|active| !active.inhibited && tags.contains_any(&active.def.immunity))
    })
}

/// The active effect on `target` that applying `id` from `source` stacks onto,
/// if there is one.
pub(super) fn stack_of(
    world: &World,
    target: Entity,
    id: EffectId,
    by: StackBy,
    source: Option<Entity>,
) -> Option<Entity> {
    effects_on(world, target).into_iter().find(|effect| {
        world.get::<ActiveEffect>(*effect).is_some_and(|active| {
            active.def.id == id && (by == StackBy::Target || active.spec.context.source == source)
        })
    })
}

/// Stacks `spec` onto `existing`, or, at the limit, applies its overflow
/// instead.
///
/// Only a stack: `existing` keeps the spec it was first applied with, and
/// `spec`'s level, params and source go only to what it chains: its
/// `on_apply`, or at the limit its overflow.
pub(super) fn stack(
    world: &mut World,
    registry: &EffectRegistry,
    existing: Entity,
    stacking: &Stacking<EffectId>,
    spec: &EffectSpec,
    depth: u32,
) {
    let Some(target) = world.get::<EffectOf>(existing).map(|of| of.0) else {
        return;
    };
    let Some(mut active) = world.get_mut::<ActiveEffect>(existing) else {
        return;
    };
    let id = active.def.id;
    if active.stacks >= stacking.limit {
        // At the limit, applying again still refreshes the time, as it says.
        if stacking.reapply == Reapply::Refresh {
            active.remaining = active.duration;
        }
        if stacking.reset_period {
            active.until_period = active.period.unwrap_or_default();
        }
        for overflow in &stacking.overflow {
            apply_at(
                world,
                target,
                *overflow,
                spec.for_effect(*overflow),
                depth + 1,
            );
        }
        world.trigger(EffectBlocked {
            entity: target,
            id,
            reason: Blocked::StackLimit,
        });
        return;
    }
    active.stacks += 1;
    if stacking.reapply == Reapply::Refresh {
        active.remaining = active.duration;
    }
    if stacking.reset_period {
        active.until_period = active.period.unwrap_or_default();
    }
    let on_apply = active.def.on_apply.clone();
    if let Some(activation) = net::predicting(world, spec) {
        if world.get::<net::Proxy>(existing).is_some()
            && let Some(mut active) = world.get_mut::<ActiveEffect>(existing)
        {
            // The server's, with a stack predicted on top: a prediction too,
            // until that stack settles.
            active.predicted += 1;
            world.entity_mut(existing).insert(Provisional);
        }
        net::stacked(world, activation, existing);
    }
    let stacks = world
        .get::<ActiveEffect>(existing)
        .map_or(0, |active| active.stacks);
    refresh(world, registry, existing);
    world.trigger(EffectStacked {
        entity: target,
        effect: existing,
        id,
        stacks,
    });
    chain(world, target, spec, &on_apply, depth);
}

/// Applies each of `effects` to `target`, as `spec` was: a chain.
pub(super) fn chain(
    world: &mut World,
    target: Entity,
    spec: &EffectSpec,
    effects: &[EffectId],
    depth: u32,
) {
    for &effect in effects {
        apply_at(world, target, effect, spec.for_effect(effect), depth + 1);
    }
}
