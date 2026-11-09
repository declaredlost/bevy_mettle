//! Ending abilities: done, cancelled, interrupted, told to the other side or
//! not.

use bevy::prelude::*;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::tag::{OwnedTags, Tags};
use bevy_mettle_effect::cue::{Cue, CueWorld};

use super::interrupts;
use crate::active::{Abilities, ActiveAbility, GrantedAbility};
use crate::behaviour::BehaviourRegistry;
use crate::events::AbilityEnded;

/// How an ability's ending goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// Told: [`AbilityEnded`] triggered, and a server tells the client whose
    /// activation it was.
    Told,
    /// Quiet: [`AbilityEnded`] triggered, and nobody told. The other side
    /// ended it, or rejected it.
    Quiet,
}

/// Ends `ability` if it's active: its behaviour taken off, the tags it granted
/// taken back; cancelled if `cancelled`. A server tells the client whose
/// activation it was; a client's ending is its own: the server's copy runs
/// its course, and the server decides.
pub(crate) fn end(world: &mut World, ability: Entity, cancelled: bool) {
    end_as(world, ability, cancelled, Ending::Told);
}

/// Ends `ability` as [`end`] does, but tells no one: the other side ended it,
/// or rejected it.
pub(crate) fn end_quietly(world: &mut World, ability: Entity, cancelled: bool) {
    end_as(world, ability, cancelled, Ending::Quiet);
}

/// Cancels `ability`, as the game asks: a client asks the server to cancel
/// its copy too, if the server runs one it does too.
pub(crate) fn cancel(world: &mut World, ability: Entity) {
    let ran = world
        .get::<ActiveAbility>(ability)
        .zip(world.get::<GrantedAbility>(ability))
        .map(|(active, granted)| (active.actor, granted.def.net, active.activation));
    end(world, ability, true);
    if let Some((actor, net, activation)) = ran {
        crate::net::ask_cancel(world, actor, net, activation);
    }
}

/// Ends `ability`, as `ending` says.
fn end_as(world: &mut World, ability: Entity, cancelled: bool, ending: Ending) {
    let (Some(granted), Some(active)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<ActiveAbility>(ability),
    ) else {
        return;
    };
    let actor = active.actor;
    let activation = active.activation;
    let (id, net) = (granted.def.id, granted.def.net);
    // What it activated with, not what its definition says now: it may have
    // been replaced since.
    let (granted_tags, cues, running) = world
        .get_mut::<GrantedAbility>(ability)
        .map(|mut granted| {
            (
                std::mem::take(&mut granted.granted),
                std::mem::take(&mut granted.cues),
                granted.running.take(),
            )
        })
        .unwrap_or_default();
    let behaviour = running.and_then(|running| {
        world
            .get_resource::<BehaviourRegistry>()
            .and_then(|behaviours| behaviours.get(running).copied())
    });
    if let Some(mut owned) = world.get_mut::<OwnedTags>(actor) {
        owned.remove_all(&granted_tags);
    }
    for cue in cues.written() {
        world.remove_cue(actor, &Cue::new(cue));
    }
    let mut entity = world.entity_mut(ability);
    entity.remove::<ActiveAbility>();
    if let Some(behaviour) = behaviour {
        (behaviour.remove)(&mut entity);
    }
    world.trigger(AbilityEnded {
        entity: actor,
        ability,
        id,
        cancelled,
    });
    if ending == Ending::Told {
        crate::net::tell_end(world, actor, net, activation, cancelled);
    }
}

/// Actors whose tags changed, to be weighed for interrupts once each as
/// commands next run, however many of their tags changed.
#[derive(Resource, Default)]
pub(crate) struct Interrupting(Vec<Entity>);

/// Notes that `actor`'s tags changed: weighed for interrupts once, as
/// commands next run. A client leaves interrupts to the server.
pub(crate) fn note_interrupt(
    actor: Entity,
    role: NetRole,
    interrupting: &mut Interrupting,
    commands: &mut Commands,
) {
    if role == NetRole::Client || interrupting.0.contains(&actor) {
        return;
    }
    if interrupting.0.is_empty() {
        commands.queue(interrupt_noted);
    }
    interrupting.0.push(actor);
}

/// Weighs every actor noted for interrupts.
fn interrupt_noted(world: &mut World) {
    let actors = std::mem::take(&mut world.resource_mut::<Interrupting>().0);
    for actor in actors {
        interrupt(world, actor);
    }
}

/// Cancels `actor`'s active abilities whose `cancel_if` holds: as its tags
/// change, as one activates, and as definitions are replaced.
///
/// A client leaves it to the server: what it predicts of its owner's tags may
/// be wrong, and the server tells it of what it interrupts.
pub(crate) fn interrupt(world: &mut World, actor: Entity) {
    if crate::net::defers(world) {
        return;
    }
    let Some(owner) = world.get::<OwnedTags>(actor) else {
        return;
    };
    let interrupted = active_where(world, actor, |granted| interrupts(granted, owner));
    for ability in interrupted {
        end(world, ability, true);
    }
}

/// Cancels `actor`'s active abilities tagged any of `tags`, or all of them if
/// none are given, as [`cancel`] does: those it cancelled.
pub(crate) fn cancel_tagged(world: &mut World, actor: Entity, tags: Option<&Tags>) -> Vec<Entity> {
    let cancelled = active_where(world, actor, |granted| {
        tags.is_none_or(|tags| granted.def.tags.contains_any(tags))
    });
    for ability in &cancelled {
        cancel(world, *ability);
    }
    cancelled
}

/// `actor`'s active abilities that `matches`: nothing's allocated for none.
fn active_where(
    world: &World,
    actor: Entity,
    matches: impl Fn(&GrantedAbility) -> bool,
) -> Vec<Entity> {
    world
        .get::<Abilities>(actor)
        .into_iter()
        .flat_map(Abilities::iter)
        .filter(|ability| {
            world.get::<ActiveAbility>(*ability).is_some()
                && world.get::<GrantedAbility>(*ability).is_some_and(&matches)
        })
        .collect()
}
