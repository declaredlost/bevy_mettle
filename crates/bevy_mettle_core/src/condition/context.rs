//! What an expression is weighed in.

use crate::Id;
use crate::tag::Tag;

/// What a [`Condition`](super::Condition) is weighed in: the read's tags (the
/// hit's, the ability's), the tags its owner, its target and its source own,
/// and the facts the read supplies. Each asks for a tag or one under it: tag
/// sets and counts have every tag above what they hold.
pub trait Context {
    /// Whether the read has `tag`: `delivery.attack`.
    fn tag(&self, tag: &Id<Tag>) -> bool;

    /// Whether the owner owns `tag`: `state.low_life`.
    fn has(&self, tag: &Id<Tag>) -> bool;

    /// Whether the target owns `tag`: `creature.undead`.
    fn target(&self, tag: &Id<Tag>) -> bool;

    /// Whether the source (the caster, the attacker) owns `tag`.
    fn source(&self, tag: &Id<Tag>) -> bool;

    /// What `fact` (a tag: `fact.flown`) is, if the read supplied it.
    fn fact(&self, fact: &Id<Tag>) -> Option<f32>;
}
