//! Playing over a network: who decides, what's predicted, what's replicated.
//!
//! Transport-agnostic: the game (or an adapter) ships [`EffectReplica`]s from
//! the server to its clients.

use std::collections::{HashMap, HashSet, VecDeque};

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_attribute::modifier::{Modifier, ModifierOf, Modifiers, Operation, Provisional};
use bevy_mettle_attribute::read::AttributeWorld;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::tag::Tags;
use serde::{Deserialize, Serialize};

use crate::active::{ActiveEffect, EffectOf};
use crate::{EffectRegistry, EffectSpec, engine};

/// On a client's active effect: predicted under an activation, until settled.
///
/// Until the server's own arrives or rejects it. It's
/// [`Provisional`] too: while it counts, its actor shows its own values, the
/// prediction, not those the server serves.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictedUnder(pub u64);

/// On a client's active effect: the server's, shown from its replica.
///
/// Not ticked here but for its time: the server's replica says how it stands.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Proxy;

/// On a server's active effect: what a client needs to show it.
///
/// For the game to replicate with its [`EffectOf`], mapping its entities. Its
/// definition is data both sides have, so it's named, not sent. Sent again as
/// it changes, but not as its time runs down: a client runs that down itself.
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectReplica {
    /// What was applied: its source mapped as it arrives.
    #[entities]
    pub spec: EffectSpec,
    /// How many stacks it has.
    pub stacks: u32,
    /// How long it lasts in full, if not forever.
    pub duration: Option<f32>,
    /// The seconds it had left as it was sent, if it doesn't last forever.
    pub remaining: Option<f32>,
    /// Whether it's inhibited.
    pub inhibited: bool,
    /// What its source owned as it applied: what source terms are settled by.
    pub source_tags: Tags,
    /// Its modifiers' attribute magnitudes, as captured as it applied.
    pub captured: Vec<Option<f32>>,
}

impl EffectReplica {
    /// Whether `self`, sent before, says all `now` says but for time run down.
    fn stands_as(&self, now: &Self) -> bool {
        let ran_down = match (self.remaining, now.remaining) {
            (Some(sent), Some(left)) => left <= sent,
            (sent, left) => sent == left,
        };
        ran_down
            && self.spec == now.spec
            && self.stacks == now.stacks
            && self.duration == now.duration
            && self.inhibited == now.inhibited
            && self.source_tags == now.source_tags
            && self.captured == now.captured
    }
}

impl MapEntities for EffectReplica {
    /// Maps its spec's source, as it arrives from another world.
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.spec.map_entities(mapper);
    }
}

/// What was done under a predicted activation, to undo.
#[derive(Debug, Clone)]
enum Undo {
    /// An effect spawned.
    Effect(Entity),
    /// A stack added to an effect.
    Stack(Entity),
    /// Instant changes to `actor`'s bases, shown as modifiers on this entity
    /// until the server's bases arrive.
    Overlay { actor: Entity, overlay: Entity },
}

/// How many answered activations are remembered, to predict nothing more
/// under them.
const ANSWERED: usize = 1024;

/// What's been done under each predicted activation, not yet answered, and the
/// activations answered.
#[derive(Resource, Debug, Default)]
pub(crate) struct Predictions {
    pending: HashMap<u64, Vec<Undo>>,
    answered: HashSet<u64>,
    /// The answered activations, oldest first, to forget the oldest.
    order: VecDeque<u64>,
}

impl Predictions {
    /// Takes what was done under `activation`, and notes it answered.
    fn answer(&mut self, activation: u64) -> Vec<Undo> {
        if self.answered.insert(activation) {
            self.order.push_back(activation);
            if self.order.len() > ANSWERED
                && let Some(oldest) = self.order.pop_front()
            {
                self.answered.remove(&oldest);
            }
        }
        self.pending.remove(&activation).unwrap_or_default()
    }
}

/// This world's role.
pub(crate) fn role(world: &World) -> NetRole {
    world.get_resource::<NetRole>().copied().unwrap_or_default()
}

/// The activation `spec` is predicted under, if this world predicts it.
pub(crate) fn predicting(world: &World, spec: &EffectSpec) -> Option<u64> {
    (role(world) == NetRole::Client && !answered(world, spec))
        .then_some(spec.activation)
        .flatten()
}

/// Whether `spec` is under an activation the server answered already, on a
/// client: what it does arrives from the server, so nothing's done here.
pub(crate) fn answered(world: &World, spec: &EffectSpec) -> bool {
    role(world) == NetRole::Client
        && spec.activation.is_some_and(|activation| {
            world
                .get_resource::<Predictions>()
                .is_some_and(|predictions| predictions.answered.contains(&activation))
        })
}

fn log(world: &mut World, activation: u64, undo: Undo) {
    world
        .get_resource_or_init::<Predictions>()
        .pending
        .entry(activation)
        .or_default()
        .push(undo);
}

/// Notes `effect` spawned under `activation`, and marks it predicted.
pub(crate) fn spawned(world: &mut World, activation: u64, effect: Entity) {
    world
        .entity_mut(effect)
        .insert((PredictedUnder(activation), Provisional));
    log(world, activation, Undo::Effect(effect));
}

/// Notes a stack added to `effect` under `activation`.
pub(crate) fn stacked(world: &mut World, activation: u64, effect: Entity) {
    log(world, activation, Undo::Stack(effect));
}

/// The overlays predicted for `actor`, under any activation not yet answered.
fn overlays(world: &World, actor: Entity) -> Vec<(u64, Entity)> {
    world
        .get_resource::<Predictions>()
        .map(|predictions| {
            predictions
                .pending
                .iter()
                .flat_map(|(activation, undo)| {
                    undo.iter().filter_map(move |undo| match undo {
                        Undo::Overlay { actor: of, overlay } if *of == actor => {
                            Some((*activation, *overlay))
                        }
                        _ => None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// How much `overlays` add to `attribute`'s base.
fn overlaid(world: &World, overlays: &[(u64, Entity)], change: &crate::code::BaseChange) -> f32 {
    overlays
        .iter()
        .filter_map(|(_, overlay)| world.get::<Modifiers>(*overlay))
        .flat_map(|modifiers| &modifiers.0)
        .filter(|modifier| modifier.attribute == change.attribute)
        .map(|modifier| modifier.value)
        .sum()
}

/// Changes `actor`'s base as an instant effect does; predicted under
/// `activation`, shown as a modifier instead: the server owns bases, and sends
/// its own.
///
/// A predicted change is worked out on the base as it's shown, every change
/// predicted before it made, and kept within its limits as the server keeps
/// it; what it comes to is shown as a flat modifier, the difference.
///
/// How much it changed the base, as it's shown.
pub(crate) fn change_base(
    world: &mut World,
    activation: Option<u64>,
    actor: Entity,
    change: &crate::code::BaseChange,
) -> f32 {
    let Some(activation) = activation else {
        let before = world.base_at(actor, &change.attribute);
        world.apply_to_base(actor, &change.attribute, change.op, change.value);
        let after = world.base_at(actor, &change.attribute);
        return after
            .zip(before)
            .map_or(0.0, |(after, before)| after - before);
    };
    let Some(base) = world.base_at(actor, &change.attribute) else {
        return 0.0;
    };
    let overlays = overlays(world, actor);
    let shown = base + overlaid(world, &overlays, change);
    let Some(after) = world.base_after(actor, &change.attribute, shown, change.op, change.value)
    else {
        return 0.0;
    };
    let difference = after - shown;
    let mine = overlays
        .iter()
        .find(|(under, _)| *under == activation)
        .map(|(_, overlay)| *overlay);
    if let Some(mut modifiers) = mine.and_then(|overlay| world.get_mut::<Modifiers>(overlay)) {
        match modifiers
            .0
            .iter_mut()
            .find(|modifier| modifier.attribute == change.attribute)
        {
            Some(modifier) => modifier.value += difference,
            None => modifiers.0.push(flat(change, difference)),
        }
    } else {
        let overlay = world
            .spawn((
                Modifiers(vec![flat(change, difference)]),
                ModifierOf(actor),
                PredictedUnder(activation),
                Provisional,
            ))
            .id();
        log(world, activation, Undo::Overlay { actor, overlay });
    }
    difference
}

/// A flat modifier of `change`'s attribute by `value`.
fn flat(change: &crate::code::BaseChange, value: f32) -> Modifier {
    Modifier::new(change.attribute.clone(), Operation::Flat, value)
}

/// The server confirmed `activation`: the effects predicted under it stay,
/// until the server's own replace them; its instant changes and the stacks it
/// added to the server's effects, shown until now, give way to the server's.
pub(crate) fn confirm(world: &mut World, activation: u64) {
    let undo = world
        .get_resource_or_init::<Predictions>()
        .answer(activation);
    let registry = world
        .get_resource::<EffectRegistry>()
        .cloned()
        .unwrap_or_default();
    for undo in undo {
        match undo {
            Undo::Overlay { overlay, .. } => despawn(world, overlay),
            Undo::Stack(effect) if world.get::<Proxy>(effect).is_some() => {
                engine::unstack(world, &registry, effect);
            }
            Undo::Stack(_) | Undo::Effect(_) => {}
        }
    }
}

fn despawn(world: &mut World, entity: Entity) {
    if world.get_entity(entity).is_ok() {
        world.despawn(entity);
    }
}

/// The server rejected `activation`: everything predicted under it undone, last
/// first.
pub(crate) fn rollback(world: &mut World, activation: u64) {
    if let Some(mut predicted) = world.get_resource_mut::<crate::cue::Predicted>() {
        predicted.forget_activation(activation);
    }
    let undo = world
        .get_resource_or_init::<Predictions>()
        .answer(activation);
    let registry = world
        .get_resource::<EffectRegistry>()
        .cloned()
        .unwrap_or_default();
    for undo in undo.into_iter().rev() {
        match undo {
            Undo::Effect(effect) => engine::remove_quietly(world, effect),
            Undo::Stack(effect) => engine::unstack(world, &registry, effect),
            Undo::Overlay { overlay, .. } => despawn(world, overlay),
        }
    }
}

/// What a client needs to show `active`.
fn replica(active: &ActiveEffect) -> EffectReplica {
    EffectReplica {
        spec: active.spec.clone(),
        stacks: active.stacks,
        duration: active.duration,
        remaining: active.remaining,
        inhibited: active.inhibited,
        source_tags: active
            .source_tags
            .iter()
            .map(|(tag, _)| tag.clone())
            .collect(),
        captured: active.captured.clone(),
    }
}

/// On a server: keeps each active effect's replica as it stands, but for its
/// time running down.
pub(crate) fn sync_replicas(
    mut commands: Commands,
    effects: Query<(Entity, &ActiveEffect, Option<&EffectReplica>), Changed<ActiveEffect>>,
) {
    for (entity, active, sent) in &effects {
        let now = replica(active);
        if !sent.is_some_and(|sent| sent.stands_as(&now)) {
            commands.entity(entity).insert(now);
        }
    }
}

/// Whether a replica arrived or changed, for [`show_replicas`] to show.
pub(crate) fn replicas_to_show(
    arrived: Query<(), (With<EffectReplica>, With<EffectOf>, Without<ActiveEffect>)>,
    changed: Query<(), Restated>,
) -> bool {
    !arrived.is_empty() || !changed.is_empty()
}

/// Effects the server's replica arrived for, not shown yet.
type Arrived = (Entity, &'static EffectReplica, &'static EffectOf);

/// Shown effects whose replicas changed.
type Restated = (With<Proxy>, Changed<EffectReplica>);

/// On a client: shows each effect the server's replica arrived for, and keeps
/// it as its replica says; a predicted one it stands for, taken off.
pub(crate) fn show_replicas(
    world: &mut World,
    arrived: &mut QueryState<Arrived, Without<ActiveEffect>>,
    changed: &mut QueryState<(Entity, &EffectReplica), Restated>,
) {
    let registry = world
        .get_resource::<EffectRegistry>()
        .cloned()
        .unwrap_or_default();
    let new: Vec<(Entity, EffectReplica, Entity)> = arrived
        .iter(world)
        .map(|(entity, replica, of)| (entity, replica.clone(), of.0))
        .collect();
    for (entity, replica, target) in new {
        engine::show(world, &registry, entity, target, &replica);
    }
    let updated: Vec<(Entity, EffectReplica)> = changed
        .iter(world)
        .map(|(entity, replica)| (entity, replica.clone()))
        .collect();
    for (entity, replica) in updated {
        engine::update_shown(world, &registry, entity, &replica);
    }
}
