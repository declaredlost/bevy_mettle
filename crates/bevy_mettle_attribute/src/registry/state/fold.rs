//! Folding actors' attributes as what counts for them changes: first
//! [`collect`] what changed, reading only, then [`fold`] it.

use std::cmp::Ordering;

use bevy::ecs::change_detection::Tick;
use bevy::ecs::system::SystemChangeTick;
use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use super::AttributeState;
use crate::modifier::{Modifier, ModifierOf, ModifierSources, Modifiers, Provisional};
use crate::registry::AttributeRegistry;

/// The actors to fold next.
#[derive(Resource, Default)]
pub(crate) struct Dirty(HashSet<Entity>);

/// Actors whose own modifiers or sources changed, or that are new.
type ChangedActor = (
    With<AttributeState>,
    Or<(
        Changed<Modifiers>,
        Changed<ModifierSources>,
        Added<AttributeState>,
    )>,
);

/// Sources whose modifiers changed, that are now another's, or that came to be
/// a prediction.
type ChangedSource = Or<(Changed<Modifiers>, Changed<ModifierOf>, Added<Provisional>)>;

/// Notes every actor whose modifiers changed.
pub(crate) fn collect(
    characters: Query<Entity, ChangedActor>,
    sources: Query<&ModifierOf, ChangedSource>,
    of: Query<&ModifierOf>,
    mut lost_modifiers: RemovedComponents<Modifiers>,
    mut lost_sources: RemovedComponents<ModifierSources>,
    mut settled: RemovedComponents<Provisional>,
    mut dirty: ResMut<Dirty>,
) {
    dirty.0.extend(characters.iter());
    dirty.0.extend(sources.iter().map(|source| source.0));
    // Modifiers taken off: the entity, whose own they were, and what it's a
    // modifier of, if it's a source too. Its last source gone: the actor.
    // A prediction no longer one: what it's a modifier of.
    for entity in lost_modifiers
        .read()
        .chain(lost_sources.read())
        .chain(settled.read())
    {
        dirty.0.insert(entity);
        if let Ok(source) = of.get(entity) {
            dirty.0.insert(source.0);
        }
    }
}

/// An actor, as folding reads and writes it.
type Actor<'a> = (
    &'a mut AttributeState,
    Option<Ref<'a, Modifiers>>,
    Option<&'a ModifierSources>,
);

/// A source, as folding reads it: its modifiers, since when they count for
/// their actor, and whether they're a prediction.
type Source<'a> = (&'a Modifiers, Ref<'a, ModifierOf>, Has<Provisional>);

/// One source's modifiers, with when it was applied: what orders them.
struct Applied<'a> {
    /// The change tick it was applied at.
    at: Tick,
    source: Entity,
    modifiers: &'a [Modifier],
}

impl Applied<'_> {
    /// Its order against `other`'s, as of `now`: the later applied after,
    /// and in one tick, by entity.
    fn order(&self, other: &Self, now: Tick) -> Ordering {
        if self.at.is_newer_than(other.at, now) {
            Ordering::Greater
        } else if other.at.is_newer_than(self.at, now) {
            Ordering::Less
        } else {
            self.source.cmp(&other.source)
        }
    }
}

/// Folds the modifiers of every actor [`collect`] noted, or every actor's when
/// the attributes there are change.
///
/// In the order they were applied, so of `Base`s and `Override`s the one
/// applied last counts: a source as its `ModifierOf` was last set, the actor's
/// own as its `Modifiers` was added; within one, in its list's order. Sources
/// applied in the same tick go in their entities' order.
pub(crate) fn fold(
    registry: Res<AttributeRegistry>,
    ticks: SystemChangeTick,
    mut dirty: ResMut<Dirty>,
    mut actors: Query<(Entity, Actor)>,
    sources: Query<Source>,
) {
    if registry.is_changed() {
        dirty.0.extend(actors.iter().map(|(actor, _)| actor));
    }
    let now = ticks.this_run();
    for actor in dirty.0.drain() {
        let Ok((_, (mut state, own, from))) = actors.get_mut(actor) else {
            continue;
        };
        let mut provisional = false;
        let mut applied: Vec<Applied> = own
            .as_ref()
            .map(|own| Applied {
                at: own.added(),
                source: actor,
                modifiers: &own.0,
            })
            .into_iter()
            .collect();
        for source in from.into_iter().flat_map(ModifierSources::iter) {
            if let Ok((modifiers, of, predicted)) = sources.get(source) {
                // A prediction with nothing to count doesn't change a value.
                provisional |= predicted && !modifiers.0.is_empty();
                applied.push(Applied {
                    at: of.last_changed(),
                    source,
                    modifiers: &modifiers.0,
                });
            }
        }
        // The first applied first, so the last applied comes last.
        applied.sort_by(|one, other| one.order(other, now));
        state.fold(
            &registry,
            applied.iter().flat_map(|applied| {
                applied
                    .modifiers
                    .iter()
                    .map(move |modifier| (applied.source, modifier))
            }),
            provisional,
        );
    }
}
