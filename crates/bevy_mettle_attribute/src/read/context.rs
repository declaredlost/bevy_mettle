//! What a read is in: its context.

use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::condition::Context;
use bevy_mettle_core::tag::{OwnedTags, Tag, TagCounts, Tags};

/// The context an attribute is read in: a hit's tags, its target, its source.
///
/// The read's tags (the hit's, the ability's), the target's owned tags, the
/// facts the read supplies, and the source involved. What a modifier's
/// [`Condition`](bevy_mettle_core::condition::Condition) is weighed in, with
/// the owner's owned tags; a local modifier counts only if its source is the
/// one named. Borrows the tag sets it's given, so a read per hit copies
/// nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct AttributeContext<'a> {
    /// The read's tags, as given.
    tags: Option<&'a Tags>,
    /// The read's tags, added one by one.
    more_tags: Tags,
    /// The target's owned tags, as given.
    target: Option<&'a TagCounts>,
    /// The target's tags, added one by one.
    more_target: Tags,
    facts: Vec<(Id<Tag>, f32)>,
    pub(crate) source: Option<Entity>,
    /// The source's owned tags, as given.
    source_owned: Option<&'a TagCounts>,
    /// The source's tags, added one by one.
    more_source: Tags,
}

impl Default for AttributeContext<'_> {
    fn default() -> Self {
        Self {
            tags: None,
            more_tags: Tags::new(),
            target: None,
            more_target: Tags::new(),
            facts: Vec::new(),
            source: None,
            source_owned: None,
            more_source: Tags::new(),
        }
    }
}

impl<'a> AttributeContext<'a> {
    /// Nothing in play: only modifiers that hold without a read count.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The read has `tags`: an ability's, a hit's.
    #[must_use]
    pub fn tags(mut self, tags: &'a Tags) -> Self {
        self.tags = Some(tags);
        self
    }

    /// The read has `tag` too.
    #[must_use]
    pub fn tag(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.more_tags.insert(tag.into());
        self
    }

    /// The target owns `owned`: its [`OwnedTags`].
    #[must_use]
    pub fn target_tags(mut self, owned: &'a OwnedTags) -> Self {
        self.target = Some(owned.counts());
        self
    }

    /// The target owns `tag` too.
    #[must_use]
    pub fn target_tag(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.more_target.insert(tag.into());
        self
    }

    /// `fact` (a tag: `fact.flown`) is `value` for this read. Supplying one
    /// again replaces it.
    #[must_use]
    pub fn fact(mut self, fact: impl Into<Id<Tag>>, value: f32) -> Self {
        let fact = fact.into();
        match self.facts.iter_mut().find(|(known, _)| *known == fact) {
            Some((_, known)) => *known = value,
            None => self.facts.push((fact, value)),
        }
        self
    }

    /// `source` involved: its local modifiers count.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// The source (the attacker, the caster) owns `owned`.
    #[must_use]
    pub fn source_tags(mut self, owned: &'a OwnedTags) -> Self {
        self.source_owned = Some(owned.counts());
        self
    }

    /// The source owns `tag` too.
    #[must_use]
    pub fn source_tag(mut self, tag: impl Into<Id<Tag>>) -> Self {
        self.more_source.insert(tag.into());
        self
    }

    /// Whether it's for nothing in particular: what an actor's cached values
    /// are read for.
    pub(crate) fn is_empty(&self) -> bool {
        self.tags.is_none_or(Tags::is_empty)
            && self.more_tags.is_empty()
            && self.target.is_none()
            && self.more_target.is_empty()
            && self.facts.is_empty()
            && self.source.is_none()
            && self.source_owned.is_none()
            && self.more_source.is_empty()
    }
}

/// A read: its context, and the owner's owned tags. What a modifier's
/// `Condition` is weighed in.
pub(crate) struct Read<'q, 'a> {
    pub(crate) context: &'q AttributeContext<'a>,
    pub(crate) owned: &'q TagCounts,
}

impl Context for Read<'_, '_> {
    fn tag(&self, tag: &Id<Tag>) -> bool {
        self.context.tags.is_some_and(|tags| tags.contains(tag))
            || self.context.more_tags.contains(tag)
    }

    fn has(&self, tag: &Id<Tag>) -> bool {
        self.owned.contains(tag)
    }

    fn target(&self, tag: &Id<Tag>) -> bool {
        self.context.target.is_some_and(|owned| owned.contains(tag))
            || self.context.more_target.contains(tag)
    }

    fn source(&self, tag: &Id<Tag>) -> bool {
        self.context
            .source_owned
            .is_some_and(|owned| owned.contains(tag))
            || self.context.more_source.contains(tag)
    }

    fn fact(&self, fact: &Id<Tag>) -> Option<f32> {
        self.context
            .facts
            .iter()
            .find(|(known, _)| known == fact)
            .map(|(_, value)| *value)
    }
}
