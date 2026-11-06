//! Folding actors' attributes as what counts for them changes: first
//! [`collect`] what changed, reading only, then [`fold`] it.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;

use super::AttributeState;
use crate::modifier::{ModifierOf, ModifierSources, Modifiers};
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

/// Sources whose modifiers changed, or that are now another's.
type ChangedSource = Or<(Changed<Modifiers>, Changed<ModifierOf>)>;

/// Notes every actor whose modifiers changed.
pub(crate) fn collect(
    characters: Query<Entity, ChangedActor>,
    sources: Query<&ModifierOf, ChangedSource>,
    of: Query<&ModifierOf>,
    mut lost_modifiers: RemovedComponents<Modifiers>,
    mut lost_sources: RemovedComponents<ModifierSources>,
    mut dirty: ResMut<Dirty>,
) {
    dirty.0.extend(characters.iter());
    dirty.0.extend(sources.iter().map(|source| source.0));
    // A source's modifiers taken off: what it's a modifier of. An actor's
    // own, or its last source gone: the actor.
    for entity in lost_modifiers.read().chain(lost_sources.read()) {
        let character = of.get(entity).map_or(entity, |source| source.0);
        dirty.0.insert(character);
    }
}

/// An actor, as folding reads and writes it.
type Actor<'a> = (
    &'a mut AttributeState,
    Option<&'a Modifiers>,
    Option<&'a ModifierSources>,
);

/// Folds the modifiers of every actor [`collect`] noted, or every actor's when
/// the attributes there are change.
pub(crate) fn fold(
    registry: Res<AttributeRegistry>,
    mut dirty: ResMut<Dirty>,
    mut actors: Query<(Entity, Actor)>,
    sources: Query<&Modifiers>,
) {
    if registry.is_changed() {
        dirty.0.extend(actors.iter().map(|(actor, _)| actor));
    }
    for actor in dirty.0.drain() {
        let Ok((_, (mut state, own, from))) = actors.get_mut(actor) else {
            continue;
        };
        let own = own
            .into_iter()
            .flat_map(|own| own.0.iter().map(move |modifier| (actor, modifier)));
        let from = from
            .into_iter()
            .flat_map(ModifierSources::iter)
            .flat_map(|source| {
                sources.get(source).into_iter().flat_map(move |modifiers| {
                    modifiers.0.iter().map(move |modifier| (source, modifier))
                })
            });
        state.fold(&registry, own.chain(from));
    }
}
