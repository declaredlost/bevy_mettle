//! Committing abilities: paying their cost, starting their cooldown, or both.

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_effect::{EffectSpec, EffectWorld};

use super::check::affords;
use super::end::end;
use crate::active::{ActiveAbility, GrantedAbility};

/// Which of an ability's commitments to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Commitment {
    /// Its cost and its cooldown.
    Both,
    /// Its cost alone.
    Cost,
    /// Its cooldown alone.
    Cooldown,
}

impl Commitment {
    /// Whether it pays the cost.
    pub(crate) fn pays(self) -> bool {
        matches!(self, Self::Both | Self::Cost)
    }

    /// Whether it starts the cooldown.
    pub(crate) fn starts_cooldown(self) -> bool {
        matches!(self, Self::Both | Self::Cooldown)
    }
}

/// What committing came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Committed {
    /// What was asked is applied: paid, cooling down, or both.
    Done,
    /// Its owner can't pay: nothing's applied.
    Unaffordable,
    /// It isn't granted, or isn't active.
    Inactive,
}

/// Applies what `commitment` says of the active `ability`'s cost and
/// cooldown, under its activation, each once: whether it went through. One
/// that can't pay its cost is cancelled; a cooldown always starts.
///
/// A client that can't pay from what it predicts leaves the ability running:
/// what it predicts may be wrong, and the server decides, ending it there and
/// telling the client if it can't pay either.
pub(crate) fn commit(world: &mut World, ability: Entity, commitment: Commitment) -> bool {
    let Some(active) = world.get::<ActiveAbility>(ability) else {
        return false;
    };
    let (actor, activation) = (active.actor, active.activation);
    match pay(world, ability, actor, activation, commitment) {
        Committed::Done => true,
        Committed::Unaffordable => {
            if !crate::net::defers(world) {
                end(world, ability, true);
            }
            false
        }
        Committed::Inactive => false,
    }
}

/// Applies what `commitment` says of `ability`'s cost and cooldown to `actor`,
/// under `activation`, each once: nothing if it can't pay. It needn't be
/// active yet: one committing as it activates pays before it starts.
pub(super) fn pay(
    world: &mut World,
    ability: Entity,
    actor: Entity,
    activation: u64,
    commitment: Commitment,
) -> Committed {
    let Some(granted) = world.get::<GrantedAbility>(ability) else {
        return Committed::Inactive;
    };
    let pay = commitment.pays() && !granted.cost_paid;
    let start = commitment.starts_cooldown() && !granted.cooldown_started;
    if pay && !affords(world, actor, granted) {
        return Committed::Unaffordable;
    }
    let level = granted.level;
    let effects: Vec<Id<_>> = granted
        .def
        .cost
        .iter()
        .filter(|_| pay)
        .chain(granted.def.cooldown.iter().filter(|_| start))
        .cloned()
        .collect();
    for effect in effects {
        world.apply_effect(
            actor,
            EffectSpec::new(effect)
                .level(level)
                .source(actor)
                .activation(activation),
        );
    }
    if let Some(mut granted) = world.get_mut::<GrantedAbility>(ability) {
        granted.cost_paid |= pay;
        granted.cooldown_started |= start;
    }
    Committed::Done
}
