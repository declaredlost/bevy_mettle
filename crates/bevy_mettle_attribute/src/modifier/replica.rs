//! Modifiers over a network: by number, as content's tags are numbered.

use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::tag::{TagId, TagRegistry};
use serde::{Deserialize, Serialize};

use super::{Modifier, ModifierOf, Modifiers};

/// On a modifier source a server replicates: its [`Modifiers`], sent by number.
///
/// On a source such as an item or a passive: its modifiers, their conditions'
/// tags by number, and the actor they count for, for the game to replicate in
/// their place.
///
/// Opt in by putting it on the source, empty: on a server, it's kept as the
/// source's modifiers and what it's [`ModifierOf`] change; on a client, the
/// source is given the modifiers and the `ModifierOf` it says, as it arrives
/// or changes. A modifier whose condition names a tag the [`TagRegistry`]
/// hasn't can't be sent: it's left out, with a warning. Active effects don't
/// need it: a client makes their modifiers from their replicas.
#[derive(
    Component,
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    Reflect
)]
#[reflect(Component, Clone, Debug, Default, PartialEq)]
pub struct ModifiersReplica {
    /// The actor its modifiers count for, if any: mapped as it arrives.
    #[entities]
    pub of: Option<Entity>,
    /// Its modifiers, their tags by number.
    pub modifiers: Vec<Modifier<TagId>>,
}

/// On a server: keeps each replicated source's replica as its modifiers, and
/// what they're of, are: as either changes, as it's put on, and as the source
/// is taken off its actor.
pub(crate) fn sync_replicas(
    role: Res<NetRole>,
    tags: Option<Res<TagRegistry>>,
    mut unlinked: RemovedComponents<ModifierOf>,
    mut sources: Query<(
        Entity,
        Option<Ref<Modifiers>>,
        Option<Ref<ModifierOf>>,
        &mut ModifiersReplica,
    )>,
) {
    let unlinked: Vec<Entity> = unlinked.read().collect();
    if *role != NetRole::Server {
        return;
    }
    let tags = tags.as_deref().cloned().unwrap_or_default();
    for (source, modifiers, of, mut replica) in &mut sources {
        let restated = replica.is_added()
            || modifiers.as_ref().is_some_and(Ref::is_changed)
            || of.as_ref().is_some_and(Ref::is_changed)
            || unlinked.contains(&source);
        if !restated {
            continue;
        }
        let modifiers = modifiers
            .iter()
            .flat_map(|modifiers| &modifiers.0)
            .filter_map(|modifier| {
                let numbered = modifier.map_tags(|tag| tags.id(tag));
                if numbered.is_none() {
                    warn!(
                        "a modifier of {source} names a tag that isn't in the tag registry: not \
                         replicated"
                    );
                }
                numbered
            })
            .collect();
        replica.set_if_neq(ModifiersReplica {
            of: of.map(|of| of.0),
            modifiers,
        });
    }
}

/// On a client: gives each replicated source the modifiers, and the actor
/// they count for, its replica says, as it arrives or changes.
pub(crate) fn show_replicas(
    role: Res<NetRole>,
    tags: Option<Res<TagRegistry>>,
    sources: Query<(Entity, &ModifiersReplica), Changed<ModifiersReplica>>,
    mut commands: Commands,
) {
    if *role != NetRole::Client {
        return;
    }
    let tags = tags.as_deref().cloned().unwrap_or_default();
    for (source, replica) in &sources {
        let modifiers: Modifiers = replica
            .modifiers
            .iter()
            .filter_map(|modifier| {
                let keyed = modifier.map_tags(|id| tags.key(id));
                if keyed.is_none() {
                    warn!(
                        "a modifier of {source} names a tag number the tag registry hasn't: left \
                         out"
                    );
                }
                keyed
            })
            .collect();
        let mut source = commands.entity(source);
        source.insert(modifiers);
        match replica.of {
            Some(actor) => {
                source.insert(ModifierOf(actor));
            }
            None => {
                source.remove::<ModifierOf>();
            }
        }
    }
}
