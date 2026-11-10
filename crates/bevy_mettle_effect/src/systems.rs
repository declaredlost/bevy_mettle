//! What runs each tick.

use bevy::prelude::*;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::tag::OwnedTags;

use crate::EffectRegistry;
use crate::active::{ActiveEffect, EffectOf};
use crate::engine::{self, Check, Due};
use crate::net::Proxy;

/// Gives active effects their new definitions, as the registry's replaced.
pub(crate) fn relink(world: &mut World, effects: &mut QueryState<Entity, With<ActiveEffect>>) {
    let registry = world.resource::<EffectRegistry>().clone();
    let all: Vec<Entity> = effects.iter(world).collect();
    for effect in all {
        engine::relink(world, &registry, effect);
    }
}

/// What's due of active effects this tick, for [`settle`] to do.
#[derive(Resource, Debug, Default)]
pub(crate) struct DueEffects(Vec<(Entity, Due)>);

/// Moves every active effect on by the tick's time, and notes those the
/// world has to look at: weighed as this tick's state is, but only as a
/// hint. [`settle`] weighs each again as it stands when it comes to it.
///
/// Time running down and periods coming round need nothing but the effect,
/// so this runs alongside the game; what changes the world is left to
/// [`settle`], which runs only when something's due.
pub(crate) fn advance(
    mut effects: Query<(Entity, &mut ActiveEffect, &EffectOf, Has<Proxy>)>,
    owners: Query<&OwnedTags>,
    role: Res<NetRole>,
    time: Res<Time>,
    mut due: ResMut<DueEffects>,
) {
    let seconds = time.delta_secs();
    let client = *role == NetRole::Client;
    for (effect, mut active, of, proxy) in &mut effects {
        if client && proxy {
            // The server's: its time runs down here, but it's taken off there.
            if !active.inhibited
                && let Some(remaining) = active.remaining
            {
                active.remaining = Some((remaining - seconds).max(0.0));
            }
            continue;
        }
        let Ok(owner) = owners.get(of.0) else {
            continue;
        };
        let check = Check {
            tags: &active.def.tags,
            owner: owner.counts(),
            source: &active.source_tags,
        };
        let holds = |condition: &Condition| condition.holds(&check);
        if active.def.end_if.as_ref().is_some_and(holds) {
            due.0.push((effect, Due { executions: 0 }));
            continue;
        }
        let ongoing = active.def.ongoing.as_ref().is_none_or(holds);
        if !ongoing {
            // Inhibited, its time stands still; counting, it's to stop.
            if !active.inhibited {
                due.0.push((effect, Due { executions: 0 }));
            }
            continue;
        }
        let mut executions = 0;
        if let Some(every) = active.period {
            active.until_period -= seconds;
            while active.until_period <= 0.0 {
                executions += 1;
                active.until_period += every;
            }
        }
        let expired = active
            .remaining
            .is_some_and(|remaining| remaining - seconds <= 0.0);
        if let Some(remaining) = active.remaining {
            active.remaining = Some(remaining - seconds);
        }
        if active.inhibited || executions > 0 || expired {
            due.0.push((effect, Due { executions }));
        }
    }
}

/// Whether anything's due of active effects this tick.
pub(crate) fn due(due: Res<DueEffects>) -> bool {
    !due.0.is_empty()
}

/// Does what's due of active effects this tick, in the order it was noted.
pub(crate) fn settle(world: &mut World) {
    let due = std::mem::take(&mut world.resource_mut::<DueEffects>().0);
    let registry = world.resource::<EffectRegistry>().clone();
    for (effect, due) in due {
        engine::settle(world, &registry, effect, due);
    }
}
