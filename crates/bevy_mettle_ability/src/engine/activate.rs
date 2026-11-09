//! Activating abilities, with their input and without, and setting new input
//! on one that's active.

use std::any::TypeId;

use bevy::prelude::*;
use bevy_mettle_core::registry::AbilityId;
use bevy_mettle_core::tag::OwnedTags;
use bevy_mettle_effect::cue::{Cue, CueWorld};

use super::check::can_activate;
use super::commit::{Commitment, Committed, pay};
use super::end::{end, end_quietly, interrupt};
use super::{abilities_of, next_activation};
use crate::active::{AbilityOf, ActiveAbility, GrantedAbility};
use crate::behaviour::{BehaviourId, BehaviourRegistry, InputChecks, NoInput};
use crate::data::{AbilityDef, Commit};
use crate::events::{AbilityActivated, AbilityFailed, Failure, GameplayEvent};

/// Activates `id` for `actor`, under `activation` if it's given: its first
/// granted ability of that id.
pub(crate) fn activate_id(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    activation: Option<u64>,
) -> Result<Entity, Failure> {
    activate_id_with::<NoInput>(world, actor, id, activation, None, None)
}

/// Activates `id` for `actor` with `input`, under `activation` if it's
/// given: its first granted ability of that id. If `behaviour` is given, the
/// ability has to be run by it: the behaviour the caller gave the input for.
pub(crate) fn activate_id_with<I: Component + Clone>(
    world: &mut World,
    actor: Entity,
    id: AbilityId,
    activation: Option<u64>,
    behaviour: Option<BehaviourId>,
    input: Option<I>,
) -> Result<Entity, Failure> {
    let granted = abilities_of(world, actor).into_iter().find(|ability| {
        world
            .get::<GrantedAbility>(*ability)
            .is_some_and(|granted| granted.def.id == id)
    });
    let Some(ability) = granted else {
        world.trigger(AbilityFailed {
            entity: actor,
            id,
            reason: Failure::NotGranted,
        });
        return Err(Failure::NotGranted);
    };
    if !runs(world, ability, behaviour) {
        failed(world, ability, Failure::Input);
        return Err(Failure::Input);
    }
    activate_with(world, ability, None, activation, input).map(|()| ability)
}

/// Whether `ability` is run by `behaviour`, if one is given; warns if not.
fn runs(world: &World, ability: Entity, behaviour: Option<BehaviourId>) -> bool {
    let (Some(behaviour), Some(granted)) = (behaviour, world.get::<GrantedAbility>(ability)) else {
        return true;
    };
    let runs = granted.def.behaviour == behaviour;
    if !runs {
        let name = |behaviour| {
            world
                .get_resource::<BehaviourRegistry>()
                .and_then(|behaviours| behaviours.name(behaviour))
                .unwrap_or("?")
        };
        warn!(
            "input for `{}` given to `{}`, which `{}` runs",
            name(behaviour),
            granted.def.name,
            name(granted.def.behaviour)
        );
    }
    runs
}

/// Activates `ability`, for `event` if one triggered it, under `activation` if
/// it's given (a client's, or the server answering it).
pub(crate) fn activate(
    world: &mut World,
    ability: Entity,
    event: Option<GameplayEvent>,
    activation: Option<u64>,
) -> Result<(), Failure> {
    activate_with::<NoInput>(world, ability, event, activation, None)
}

/// Activates `ability` with `input`, its behaviour's: as [`activate`] does,
/// the input checked first, and put on it next to its behaviour.
pub(crate) fn activate_with<I: Component + Clone>(
    world: &mut World,
    ability: Entity,
    event: Option<GameplayEvent>,
    activation: Option<u64>,
    input: Option<I>,
) -> Result<(), Failure> {
    let checked =
        can_activate(world, ability).and_then(|()| input_holds(world, ability, input.as_ref()));
    if let Err(reason) = checked {
        failed(world, ability, reason);
        return Err(reason);
    }
    let Some(actor) = world.get::<AbilityOf>(ability).map(|of| of.0) else {
        return Err(Failure::NotGranted);
    };
    let Some(granted) = world.get::<GrantedAbility>(ability) else {
        return Err(Failure::NotGranted);
    };
    let def = granted.def.clone();
    let level = granted.level;
    let activation = match activation {
        Some(activation) => activation,
        None => next_activation(world),
    };
    if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
        granted.cost_paid = false;
        granted.cooldown_started = false;
    }
    // Paid before anything starts: one that can't pay leaves nothing behind.
    if def.commit == Commit::OnActivate
        && pay(world, ability, actor, activation, Commitment::Both) == Committed::Unaffordable
    {
        failed(world, ability, Failure::Cost);
        return Err(Failure::Cost);
    }
    // Only what a client's told of needs the input again.
    let tells = crate::net::tells_start(world, &def);
    let told = if tells { input.clone() } else { None };
    start(world, ability, actor, &def, level, event, activation, input);
    if tells {
        crate::net::started(world, actor, &def, activation, told);
    }
    Ok(())
}

/// Starts `ability` on a client as the server says it started there, under
/// the server's `activation`, with its `input`: not checked or paid again,
/// the server having done both. What a [`ServerInitiated`] ability's owning
/// client runs.
///
/// [`ServerInitiated`]: crate::data::NetExecution::ServerInitiated
pub(crate) fn mirror<I: Component + Clone>(
    world: &mut World,
    ability: Entity,
    activation: u64,
    input: Option<I>,
) {
    let Some(actor) = world.get::<AbilityOf>(ability).map(|of| of.0) else {
        return;
    };
    if world.get::<ActiveAbility>(ability).is_some() {
        // Running already: the server started it again, after its end.
        end_quietly(world, ability, true);
    }
    let Some(granted) = world.get::<GrantedAbility>(ability) else {
        return;
    };
    let (def, level) = (granted.def.clone(), granted.level);
    start(world, ability, actor, &def, level, None, activation, input);
}

/// Starts `ability`, checked and paid for: what it cancels ended, its tags
/// granted, its cues played, and its behaviour put on with its input.
#[expect(
    clippy::too_many_arguments,
    reason = "what an activation starts with, in one place"
)]
fn start<I: Component + Clone>(
    world: &mut World,
    ability: Entity,
    actor: Entity,
    def: &AbilityDef,
    level: f32,
    event: Option<GameplayEvent>,
    activation: u64,
    input: Option<I>,
) {
    let cancelled: Vec<Entity> = abilities_of(world, actor)
        .into_iter()
        .filter(|other| {
            *other != ability
                && world.get::<ActiveAbility>(*other).is_some()
                && world
                    .get::<GrantedAbility>(*other)
                    .is_some_and(|other| other.def.tags.contains_any(&def.cancels))
        })
        .collect();
    for other in cancelled {
        end(world, other, true);
    }
    if let Some(mut owned) = world.get_mut::<OwnedTags>(actor) {
        owned.add_all(&def.grants);
    }
    for cue in def.cues.written() {
        world.add_cue(actor, &Cue::new(cue));
    }
    if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
        granted.granted = def.grants.clone();
        granted.cues = def.cues.clone();
        granted.running = Some(def.behaviour);
    }
    let behaviour = world
        .get_resource::<BehaviourRegistry>()
        .and_then(|behaviours| behaviours.get(def.behaviour).copied());
    let mut entity = world.entity_mut(ability);
    entity.insert(ActiveAbility {
        ability: def.id,
        actor,
        activation,
        level,
        event,
    });
    if let Some(behaviour) = behaviour {
        (behaviour.insert)(&mut entity);
    }
    if let Some(input) = input {
        entity.insert(input);
    }
    world.trigger(AbilityActivated {
        entity: actor,
        ability,
        id: def.id,
        activation,
    });
    // What it paid, granted or cancelled may interrupt it.
    interrupt(world, actor);
}

/// Tells that `ability` didn't activate, and why.
fn failed(world: &mut World, ability: Entity, reason: Failure) {
    if let (Some(granted), Some(of)) = (
        world.get::<GrantedAbility>(ability),
        world.get::<AbilityOf>(ability),
    ) {
        let id = granted.def.id;
        let actor = of.0;
        world.trigger(AbilityFailed {
            entity: actor,
            id,
            reason,
        });
    }
}

/// Whether an `I` is what `ability`'s behaviour takes: none, if it takes
/// none.
fn input_fits<I: 'static>(world: &World, ability: Entity, given: bool) -> bool {
    let takes = world.get::<GrantedAbility>(ability).and_then(|granted| {
        world
            .get_resource::<BehaviourRegistry>()?
            .get(granted.def.behaviour)?
            .input
    });
    takes.map(|input| input.type_id) == given.then(TypeId::of::<I>)
}

/// Whether `input` passes its check, if it has one, for `ability`.
fn passes<I: 'static>(world: &mut World, ability: Entity, input: &I) -> bool {
    let Some(actor) = world.get::<AbilityOf>(ability).map(|of| of.0) else {
        return false;
    };
    let Some(check) = world
        .get_resource::<InputChecks>()
        .and_then(|checks| checks.0.get(&TypeId::of::<I>()).cloned())
    else {
        return true;
    };
    check(world, actor, ability, input)
}

/// Whether `input` is what `ability`'s behaviour takes, and passes its check.
fn input_holds<I: Component + Clone>(
    world: &mut World,
    ability: Entity,
    input: Option<&I>,
) -> Result<(), Failure> {
    if !input_fits::<I>(world, ability, input.is_some()) {
        return Err(Failure::Input);
    }
    match input {
        Some(input) if !passes(world, ability, input) => Err(Failure::Input),
        _ => Ok(()),
    }
}

/// Gives the active `ability` `input`, in place of what it had.
///
/// On a client, predicted and sent to the server, which decides. Where it's
/// decided, checked first: input that isn't its behaviour's, or doesn't
/// pass, cancels the ability, and on a server is rejected back to the client.
pub(crate) fn set_input<I: Component + Clone>(
    world: &mut World,
    ability: Entity,
    behaviour: Option<BehaviourId>,
    input: I,
) -> Result<(), Failure> {
    let (Some(active), Some(granted)) = (
        world.get::<ActiveAbility>(ability),
        world.get::<GrantedAbility>(ability),
    ) else {
        warn!("input given to {ability}, which isn't an active ability");
        return Err(Failure::Input);
    };
    let (actor, activation, id, name) = (
        active.actor,
        active.activation,
        granted.def.id,
        granted.def.name.clone(),
    );
    let fits = runs(world, ability, behaviour) && input_fits::<I>(world, ability, true);
    if crate::net::defers(world) {
        if !fits {
            warn!(
                "{} given to `{name}`, whose behaviour doesn't take it",
                std::any::type_name::<I>()
            );
            return Err(Failure::Input);
        }
        world.entity_mut(ability).insert(input.clone());
        crate::net::send_input(world, actor, activation, input);
        return Ok(());
    }
    if !(fits && passes(world, ability, &input)) {
        // The rejection tells the client, which undoes it.
        end_quietly(world, ability, true);
        world.trigger(AbilityFailed {
            entity: actor,
            id,
            reason: Failure::Input,
        });
        crate::net::reject(world, actor, id, activation, Failure::Input);
        return Err(Failure::Input);
    }
    world.entity_mut(ability).insert(input);
    Ok(())
}
