//! What abilities do as the world changes: events sent, tags owned and let go,
//! effects applied, grants taken back.

use bevy::prelude::*;
use bevy_mettle_core::tag::TagKey;

use super::activate::activate;
use super::end::end;
use super::{abilities_of, grant, revoke};
use crate::AbilityRegistry;
use crate::active::GrantedAbility;
use crate::data::Trigger;
use crate::events::GameplayEvent;

/// Sends `event` to its actor: told, and the abilities it triggers activated
/// with it.
pub(crate) fn send(world: &mut World, event: GameplayEvent) {
    let actor = event.entity;
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted.def.triggers.iter().any(|trigger| {
                    matches!(trigger, Trigger::Event(tag) if event.tag.is_within(*tag))
                })
                })
        })
        .collect();
    world.trigger(event.clone());
    if crate::net::defers(world) {
        return;
    }
    for ability in triggered {
        let _ = activate(world, ability, Some(event.clone()), None);
    }
}

/// `actor` came to own `tag`: abilities it triggers activated.
pub(crate) fn tag_added(world: &mut World, actor: Entity, tag: TagKey) {
    if crate::net::defers(world) {
        return;
    }
    let triggered: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|ability| {
            world
                .get::<GrantedAbility>(*ability)
                .is_some_and(|granted| {
                    granted.def.triggers.iter().any(|trigger| match trigger {
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
