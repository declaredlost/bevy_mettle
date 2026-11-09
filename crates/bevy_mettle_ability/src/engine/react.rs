//! What abilities do as the world changes: events sent, tags owned and let go,
//! effects applied, grants taken back.

use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::AbilityId;
use bevy_mettle_core::tag::TagKey;

use super::activate::activate;
use super::end::end;
use super::{abilities_of, grant, revoke};
use crate::AbilityRegistry;
use crate::active::GrantedAbility;
use crate::data::{NetExecution, Trigger};
use crate::events::GameplayEvent;

/// Sends `event` to its actor: told, and the abilities it triggers activated
/// with it.
///
/// Sent under a predicted activation, a [`Predicted`] ability it triggers
/// runs under an activation derived from it, the same on a client and its
/// server: a client predicts it at once, and the server answers it. Others it
/// triggers, the server decides alone, as it decides an event sent by no
/// ability.
///
/// [`Predicted`]: crate::data::NetExecution::Predicted
pub(crate) fn send(world: &mut World, event: GameplayEvent) {
    let actor = event.entity;
    let triggered: Vec<(Entity, AbilityId, NetExecution)> = abilities_of(world, actor)
        .into_iter()
        .filter_map(|ability| {
            let granted = world.get::<GrantedAbility>(ability)?;
            let triggers =
                granted.def.triggers.iter().any(
                    |trigger| matches!(trigger, Trigger::Event(tag) if event.tag.is_within(*tag)),
                );
            triggers.then_some((ability, granted.def.id, granted.def.net))
        })
        .collect();
    world.trigger(event.clone());
    let role = crate::net::role(world);
    for (ability, id, net) in triggered {
        // A local-only ability runs where it's triggered, whatever this is.
        if net == NetExecution::LocalOnly {
            let _ = activate(world, ability, Some(event.clone()), None);
            continue;
        }
        let parent = event.activation.filter(|_| net == NetExecution::Predicted);
        let activation = parent.map(|parent| crate::net::derived(parent, id));
        match role {
            // A client predicts only what's triggered under its own prediction:
            // under the server's activation, the server decides.
            NetRole::Client => {
                let (Some(parent), Some(activation)) = (parent, activation) else {
                    continue;
                };
                if parent & crate::net::SERVERS != 0 {
                    continue;
                }
                if activate(world, ability, Some(event.clone()), Some(activation)).is_ok() {
                    world
                        .get_resource_or_init::<crate::net::Pending>()
                        .insert_derived(parent, activation, ability);
                }
            }
            NetRole::Server => {
                let result = activate(world, ability, Some(event.clone()), activation);
                // Derived from a client's: answered, as the client predicts it.
                if let Some(activation) =
                    activation.filter(|activation| activation & crate::net::SERVERS == 0)
                {
                    crate::net::answer(world, actor, id, activation, result.err());
                }
            }
            NetRole::Standalone => {
                let _ = activate(world, ability, Some(event.clone()), activation);
            }
        }
    }
}

/// `actor` came to own `tag`: abilities it triggers activated.
pub(crate) fn tag_added(world: &mut World, actor: Entity, tag: TagKey) {
    // A client leaves tags triggering to the server, but for local-only
    // abilities, which run where they're triggered.
    let defers = crate::net::defers(world);
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    (!defers || granted.def.net == NetExecution::LocalOnly)
                        && granted.def.triggers.iter().any(|trigger| match trigger {
                            Trigger::TagAdded(on) | Trigger::TagPresent(on) => *on == tag,
                            Trigger::Event(_) => false,
                        })
                })
        })
        .collect();
    for ability in triggered {
        let _ = activate(world, ability, None, None);
    }
}

/// `actor` no longer owns `tag`: abilities present on it cancelled.
pub(crate) fn tag_removed(world: &mut World, actor: Entity, tag: TagKey) {
    if crate::net::defers(world) {
        return;
    }
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted
                        .def
                        .triggers
                        .iter()
                        .any(|trigger| matches!(trigger, Trigger::TagPresent(on) if *on == tag))
                })
        })
        .collect();
    for ability in triggered {
        end(world, ability, true);
    }
}

/// An effect applied to `actor`: the abilities it grants, granted.
pub(crate) fn effect_applied(world: &mut World, actor: Entity, effect: Entity) {
    if crate::net::defers(world) {
        return;
    }
    let Some(active) = world.get::<bevy_mettle_effect::active::ActiveEffect>(effect) else {
        return;
    };
    let level = active.spec().level;
    let Some(registry) = world.get_resource::<AbilityRegistry>() else {
        return;
    };
    let abilities = registry.granted_by(active.id()).to_vec();
    for ability in abilities {
        grant(world, actor, ability, level, Some(effect));
    }
}

/// The effect `ability` was granted by is gone: it's taken away too.
pub(crate) fn ungranted(world: &mut World, ability: Entity) {
    if world.get_entity(ability).is_ok() {
        revoke(world, ability);
    }
}
