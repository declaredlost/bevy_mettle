//! Live reads: a lasting effect following what its live captures read, as
//! it changes.

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy_mettle_attribute::AttributeFold;
use bevy_mettle_attribute::registry::AttributeState;
use bevy_mettle_core::tag::OwnedTags;

use super::count::refresh;
use super::execution::read;
use crate::EffectRegistry;
use crate::active::{ActiveEffect, EffectOf};
use crate::code::ExecutionContext;
use crate::data::Side;

/// On a lasting effect with live captures: whose each reads, what it read
/// last, and the actors it follows.
#[derive(Component, Debug, Clone, PartialEq)]
#[component(on_insert = follow, on_discard = unfollow)]
pub(crate) struct LiveReads {
    /// Each live capture's slot, and whose it reads.
    reads: Vec<(usize, Entity)>,
    /// What each read last.
    last: Vec<Option<f32>>,
    /// Every actor what it reads depends on, each once: whose attributes it
    /// reads, and its target and source, whose owned tags the reads, read as
    /// a hit's, count conditions by.
    followed: Vec<Entity>,
}

/// On an actor a lasting effect's live reads depend on: the effects following
/// it.
#[derive(Component, Debug, Clone, Default, PartialEq)]
pub(crate) struct FollowedBy(Vec<Entity>);

/// Notes `effect` following every actor its live reads depend on.
fn follow(mut world: DeferredWorld, context: HookContext) {
    let Some(reads) = world.get::<LiveReads>(context.entity) else {
        return;
    };
    let followed = reads.followed.clone();
    let effect = context.entity;
    let mut commands = world.commands();
    for whose in followed {
        commands.queue(move |world: &mut World| {
            if let Ok(mut entity) = world.get_entity_mut(whose) {
                let mut followed = entity.entry::<FollowedBy>().or_default();
                let followed = &mut followed.get_mut().0;
                if !followed.contains(&effect) {
                    followed.push(effect);
                }
            }
        });
    }
}

/// Notes `effect` no longer following the actors its live reads depend on.
fn unfollow(mut world: DeferredWorld, context: HookContext) {
    let Some(reads) = world.get::<LiveReads>(context.entity) else {
        return;
    };
    let followed = reads.followed.clone();
    let effect = context.entity;
    for whose in followed {
        if let Some(mut followed) = world.get_mut::<FollowedBy>(whose) {
            followed.0.retain(|known| *known != effect);
        }
    }
}

/// Puts on `effect` what its live captures read, if it lasts and has any;
/// takes it off if not.
pub(crate) fn watch(world: &mut World, effect: Entity) {
    let reads = world
        .get::<ActiveEffect>(effect)
        .zip(world.get::<EffectOf>(effect))
        .filter(|(active, _)| active.def.lasts())
        .map(|(active, of)| {
            let context = ExecutionContext {
                world,
                target: of.0,
                def: &active.def,
                spec: &active.spec,
                stacks: active.stacks,
                captured: &active.captured,
                reads: &[],
            };
            let reads: Vec<(usize, Entity)> = active
                .def
                .captures
                .iter()
                .enumerate()
                .filter(|(_, capture)| !capture.snapshot)
                .filter_map(|(slot, capture)| {
                    let whose = match capture.from {
                        Side::Source => active.spec.context.source?,
                        Side::Target => of.0,
                    };
                    Some((slot, whose))
                })
                .collect();
            let last = reads
                .iter()
                .map(|&(slot, _)| read(&context, &active.def.captures[slot]))
                .collect();
            let mut followed: Vec<Entity> = reads
                .iter()
                .map(|&(_, whose)| whose)
                .chain([of.0])
                .chain(active.spec.context.source)
                .collect();
            followed.sort_unstable();
            followed.dedup();
            LiveReads {
                reads,
                last,
                followed,
            }
        })
        .filter(|reads| !reads.reads.is_empty());
    let Ok(mut entity) = world.get_entity_mut(effect) else {
        return;
    };
    match reads {
        Some(reads) => {
            entity.insert(reads);
        }
        None => {
            entity.remove::<LiveReads>();
        }
    }
}

/// Actors whose values or owned tags changed: what a live read, read as a
/// hit's, depends on.
type Followed = Or<(Changed<AttributeState>, Changed<OwnedTags>)>;

/// How many rounds live reads settle in before what's left follows on the
/// next step: what bounds effects that feed each other and don't settle.
const ROUNDS: usize = 8;

/// Settles every live read before values are published: re-counts each
/// effect whose reads moved, folds attributes again, and follows what that
/// moved in turn, until nothing does.
///
/// So an aura follows its caster's spell power in the step it changed, and
/// what's published is settled: never a value on its way to another.
pub(crate) fn settle_live(
    world: &mut World,
    changed: &mut QueryState<&FollowedBy, Followed>,
    mut unsettled: Local<Vec<Entity>>,
) {
    let registry = world
        .get_resource::<EffectRegistry>()
        .cloned()
        .unwrap_or_default();
    // What changed since this last ran: the step's folding, and anything
    // since the step before. What it folded itself, it doesn't see changed:
    // what it left unsettled, it carries.
    let mut effects = followers(changed.iter(world));
    effects.append(&mut unsettled);
    effects.sort_unstable();
    effects.dedup();
    for _ in 0..ROUNDS {
        if effects.is_empty() || !recount(world, &registry, effects) {
            return;
        }
        // Fold what that changed, and follow only what that moved.
        let since = world.change_tick();
        if world.try_run_schedule(AttributeFold).is_err() {
            return;
        }
        effects = world.last_change_tick_scope(since, |world| followers(changed.iter(world)));
    }
    *unsettled = effects;
    if crate::context::first_warning("unsettled live reads".to_owned()) {
        warn!(
            "live reads moved still after {ROUNDS} rounds: what's left follows on the next step \
             (effects reading each other live that don't settle)"
        );
    }
}

/// The effects following `followed`, each once.
fn followers<'a>(followed: impl Iterator<Item = &'a FollowedBy>) -> Vec<Entity> {
    let mut effects: Vec<Entity> = followed
        .flat_map(|followed| followed.0.iter().copied())
        .collect();
    effects.sort_unstable();
    effects.dedup();
    effects
}

/// Re-counts each of `effects` whose live reads moved: whether any did.
fn recount(world: &mut World, registry: &EffectRegistry, effects: Vec<Entity>) -> bool {
    let mut any = false;
    for effect in effects {
        let now = world
            .get::<ActiveEffect>(effect)
            .zip(world.get::<EffectOf>(effect))
            .zip(world.get::<LiveReads>(effect))
            .map(|((active, of), live)| {
                let context = ExecutionContext {
                    world,
                    target: of.0,
                    def: &active.def,
                    spec: &active.spec,
                    stacks: active.stacks,
                    captured: &active.captured,
                    reads: &[],
                };
                live.reads
                    .iter()
                    .map(|&(slot, _)| read(&context, &active.def.captures[slot]))
                    .collect::<Vec<_>>()
            });
        let Some(now) = now else {
            continue;
        };
        let Some(mut live) = world.get_mut::<LiveReads>(effect) else {
            continue;
        };
        let moved = now
            .iter()
            .zip(&live.last)
            .any(|(now, last)| now.map(f32::to_bits) != last.map(f32::to_bits));
        if moved {
            live.bypass_change_detection().last = now;
            refresh(world, registry, effect);
            any = true;
        }
    }
    any
}
