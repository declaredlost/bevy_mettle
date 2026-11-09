//! Applying, executing, stacking, inhibiting and removing effects: what
//! commands and the tick run, on the world.

mod apply;
mod count;
mod execution;
mod remove;
mod show;
mod tick;

use bevy::prelude::*;
use bevy_mettle_core::condition::Context;
use bevy_mettle_core::tag::{OwnedTags, TagCounts, TagKey, Tags};

pub(crate) use self::apply::apply;
pub(crate) use self::execution::preview;
pub(crate) use self::remove::{
    remove, remove_activation, remove_quietly, remove_tagged, removed_unasked,
};
pub(crate) use self::show::{relink, show, unstack, update_shown};
pub(crate) use self::tick::{Due, settle};
use crate::active::ActiveEffects;

/// How deep effects applying effects go before it's taken for a loop.
pub(super) const DEPTH: u32 = 16;

/// What an effect's requirements and executing modifiers are weighed in: its
/// own tags, its target's (as `OwnerHas` and `TargetHas`), and its source's as
/// it applied.
pub(crate) struct Check<'a> {
    pub(crate) tags: &'a Tags,
    pub(crate) owner: &'a TagCounts,
    pub(crate) source: &'a TagCounts,
}

impl Context for Check<'_> {
    fn tag(&self, tag: TagKey) -> bool {
        self.tags.contains(tag)
    }

    fn has(&self, tag: TagKey) -> bool {
        self.owner.contains(tag)
    }

    fn target(&self, tag: TagKey) -> bool {
        self.owner.contains(tag)
    }

    fn source(&self, tag: TagKey) -> bool {
        self.source.contains(tag)
    }

    fn fact(&self, _fact: TagKey) -> Option<f32> {
        None
    }
}

/// `stacks` as a number to multiply by: past 65535, as many.
pub(super) fn count(stacks: u32) -> f32 {
    f32::from(u16::try_from(stacks).unwrap_or(u16::MAX))
}

/// The effects on `target`.
pub(super) fn effects_on(world: &World, target: Entity) -> Vec<Entity> {
    world
        .get::<ActiveEffects>(target)
        .map(|effects| effects.iter().collect())
        .unwrap_or_default()
}

/// What `source` owns, if it's there.
pub(super) fn source_tags(world: &World, source: Option<Entity>) -> TagCounts {
    source
        .and_then(|source| world.get::<OwnedTags>(source))
        .map(|owned| owned.counts().clone())
        .unwrap_or_default()
}
