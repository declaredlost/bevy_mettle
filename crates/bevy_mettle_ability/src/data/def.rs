//! Abilities as an app has them: checked, what their data names resolved to
//! numbers.

use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::{AbilityId, AnyAbility, EffectId, Key};
use bevy_mettle_core::tag::Tags;

use super::{Commit, NetExecution, Trigger};
use crate::behaviour::BehaviourId;

/// An ability as an app has it: its data checked, and what it names numbered.
///
/// Its [`AbilityData`](super::AbilityData) checked, the behaviour and effects
/// it names resolved to their numbers.
///
/// What [`AbilityRegistry`](crate::AbilityRegistry) holds, and a granted
/// ability runs by.
#[derive(Debug, Clone, PartialEq)]
pub struct AbilityDef {
    /// Its number.
    pub id: AbilityId,
    /// Its name.
    pub name: Key<AnyAbility>,
    /// What it's labelled.
    pub tags: Tags,
    /// The behaviour that runs it.
    pub behaviour: BehaviourId,
    /// What it costs: an instant effect on its owner.
    pub cost: Option<EffectId>,
    /// Its cooldown: a lasting effect granting a tag.
    pub cooldown: Option<EffectId>,
    /// What has to hold for it to activate.
    pub require: Option<Condition>,
    /// What interrupts it.
    pub cancel_if: Option<Condition>,
    /// It can't activate while its owner owns any of these.
    pub blocked_by: Tags,
    /// The active abilities it cancels as it activates.
    pub cancels: Tags,
    /// The abilities it keeps from activating while it's active.
    pub blocks: Tags,
    /// What its owner owns while it's active.
    pub grants: Tags,
    /// The cues it plays while it's active.
    pub cues: Tags,
    /// What activates it.
    pub triggers: Vec<Trigger>,
    /// When it pays its cost and starts its cooldown.
    pub commit: Commit,
    /// How it runs over a network.
    pub net: NetExecution,
}
