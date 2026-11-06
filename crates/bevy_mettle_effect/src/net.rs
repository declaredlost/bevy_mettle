//! Playing over a network: who decides, what's predicted, and what's
//! replicated. Transport-agnostic: the game (or an adapter) ships
//! [`EffectReplica`]s from the server to its clients.

use std::collections::HashMap;

use bevy::prelude::*;
use bevy_mettle_attribute::{AttributeWorld, Modifier, ModifierOf, Modifiers, Operation};
use serde::{Deserialize, Serialize};

use crate::{ActiveEffect, EffectOf, EffectRegistry, EffectSpec, engine};

/// What this world is, playing over a network.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum NetRole {
    /// Playing alone: everything decided here, nothing replicated.
    #[default]
    Standalone,
    /// The server: everything decided here; active effects keep an
    /// [`EffectReplica`] for clients.
    Server,
    /// A client: effects the server owns are shown from their replicas,
    /// not ticked; what's applied under an activation is predicted, kept
    /// until the server confirms it or undone when it rejects it. A
    /// predicted instant change is shown as a modifier, never written to a
    /// base: the server owns bases, and its own replace it either way.
    Client,
}

/// On a client's active effect: predicted, under this activation, until
/// the server's own arrives or rejects it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PredictedUnder(pub u64);

/// On a client's active effect: the server's, shown from its replica. Not
/// ticked here: the server's replica says how it stands.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Proxy;

/// On a server's active effect: what a client needs to show it, for the
/// game to replicate with its [`EffectOf`]. Its definition is data both
/// sides have, so it's named, not sent.
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectReplica {
    /// What was applied.
    pub spec: EffectSpec,
    /// How many stacks it has.
    pub stacks: u32,
    /// The seconds it has left, if it doesn't last forever.
    pub remaining: Option<f32>,
    /// Whether it's inhibited.
    pub inhibited: bool,
}

/// What was done under a predicted activation, to undo.
#[derive(Debug, Clone)]
enum Undo {
    /// An effect spawned.
    Effect(Entity),
    /// A stack added to an effect.
    Stack(Entity),
    /// Instant changes to `actor`'s bases, shown as modifiers on this
    /// entity until the server's bases arrive.
    Overlay { actor: Entity, overlay: Entity },
}

/// What's been done under each predicted activation, not yet confirmed.
#[derive(Resource, Debug, Default)]
pub(crate) struct Predictions(HashMap<u64, Vec<Undo>>);

/// This world's role.
pub(crate) fn role(world: &World) -> NetRole {
    world.get_resource::<NetRole>().copied().unwrap_or_default()
}

/// The activation `spec` is predicted under, if this world predicts.
pub(crate) fn predicting(world: &World, spec: &EffectSpec) -> Option<u64> {
    (role(world) == NetRole::Client)
        .then_some(spec.activation)
        .flatten()
}

fn log(world: &mut World, activation: u64, undo: Undo) {
    world
        .get_resource_or_init::<Predictions>()
        .0
        .entry(activation)
        .or_default()
        .push(undo);
}

/// Notes `effect` spawned under `activation`, and marks it predicted.
pub(crate) fn spawned(world: &mut World, activation: u64, effect: Entity) {
    world.entity_mut(effect).insert(PredictedUnder(activation));
    log(world, activation, Undo::Effect(effect));
}

/// Notes a stack added to `effect` under `activation`.
pub(crate) fn stacked(world: &mut World, activation: u64, effect: Entity) {
    log(world, activation, Undo::Stack(effect));
}

/// Changes `actor`'s base as an instant effect does; predicted under
/// `activation`, shown as a modifier instead: the server owns bases, and
/// sends its own.
pub(crate) fn change_base(
    world: &mut World,
    activation: Option<u64>,
    actor: Entity,
    change: &crate::BaseChange,
) {
    let Some(activation) = activation else {
        world.apply_to_base(actor, &change.attribute, change.op, change.value);
        return;
    };
    let modifier = Modifier {
        attribute: change.attribute.clone(),
        op: match change.op {
            Operation::Override => Operation::Base,
            op => op,
        },
        value: change.value,
        condition: None,
        per: None,
        local: false,
    };
    let existing = world
        .get_resource::<Predictions>()
        .and_then(|predictions| predictions.0.get(&activation))
        .and_then(|undo| {
            undo.iter().find_map(|undo| match undo {
                Undo::Overlay { actor: of, overlay } if *of == actor => Some(*overlay),
                _ => None,
            })
        });
    if let Some(mut modifiers) = existing.and_then(|overlay| world.get_mut::<Modifiers>(overlay)) {
        modifiers.0.push(modifier);
    } else {
        let overlay = world
            .spawn((
                Modifiers(vec![modifier]),
                ModifierOf(actor),
                PredictedUnder(activation),
            ))
            .id();
        log(world, activation, Undo::Overlay { actor, overlay });
    }
}

/// The server confirmed `activation`: the effects predicted under it stay,
/// until the server's own replace them; its instant changes, shown until
/// now, give way to the server's bases.
pub(crate) fn confirm(world: &mut World, activation: u64) {
    let undo = world
        .get_resource_mut::<Predictions>()
        .and_then(|mut predictions| predictions.0.remove(&activation))
        .unwrap_or_default();
    for undo in undo {
        if let Undo::Overlay { overlay, .. } = undo {
            despawn(world, overlay);
        }
    }
}

fn despawn(world: &mut World, entity: Entity) {
    if world.get_entity(entity).is_ok() {
        world.despawn(entity);
    }
}

/// The server rejected `activation`: everything predicted under it undone,
/// last first.
pub(crate) fn rollback(world: &mut World, activation: u64) {
    let undo = world
        .get_resource_mut::<Predictions>()
        .and_then(|mut predictions| predictions.0.remove(&activation))
        .unwrap_or_default();
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

/// On a server: keeps each active effect's replica as it stands.
pub(crate) fn sync_replicas(
    mut commands: Commands,
    effects: Query<(Entity, &ActiveEffect, Option<&EffectReplica>), Changed<ActiveEffect>>,
) {
    for (entity, active, replica) in &effects {
        let now = EffectReplica {
            spec: active.spec.clone(),
            stacks: active.stacks,
            remaining: active.remaining,
            inhibited: active.inhibited,
        };
        if replica != Some(&now) {
            commands.entity(entity).insert(now);
        }
    }
}

/// Effects the server's replica arrived for, not shown yet.
type Arrived = (Entity, &'static EffectReplica, &'static EffectOf);

/// Shown effects whose replicas changed.
type Restated = (With<Proxy>, Changed<EffectReplica>);

/// On a client: shows each effect the server's replica arrived for, and
/// keeps it as its replica says; a predicted one it stands for, taken off.
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
