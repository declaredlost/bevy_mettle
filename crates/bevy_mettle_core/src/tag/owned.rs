//! The tags an actor owns, what's told as they come and go, and a guard on
//! them.

use std::ops::Deref;

use bevy_ecs::prelude::*;
use bevy_ecs::reflect::ReflectComponent;
use bevy_reflect::prelude::*;

use super::{TagCounts, TagKey, Tags};

/// On an actor: the tags it owns, by count, and every tag above them.
///
/// Its states: what the game marks it with (`state.moving`, `state.low_life`)
/// and what effects grant it (`immune.stun`). Each source adds one and takes
/// its one away. Reads consult it, so nothing folds again when it changes.
///
/// A tag coming to be owned, or ceasing to be (it, or one above it), is told as
/// [`TagAdded`] or [`TagRemoved`] as [`publish_tags`] runs: as
/// `bevy_mettle_attribute` schedules it.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Clone, Debug, Default, PartialEq)]
pub struct OwnedTags {
    counts: TagCounts,
    /// Tags that came to be owned (`true`) or ceased to be, not yet told.
    #[reflect(ignore)]
    changes: Vec<(TagKey, bool)>,
}

impl OwnedTags {
    /// One more source of `tag`.
    pub fn add(&mut self, tag: impl Into<TagKey>) {
        let tag = tag.into();
        for tag in tag.lineage() {
            if !self.counts.contains(tag) {
                self.changes.push((tag, true));
            }
        }
        self.counts.add(tag);
    }

    /// One fewer source of `tag`: gone with its last.
    ///
    /// Nothing for a tag not given, even one owned under another.
    pub fn remove(&mut self, tag: impl Into<TagKey>) {
        let tag = tag.into();
        if !self.counts.remove(tag) {
            return;
        }
        for tag in tag.lineage() {
            if !self.counts.contains(tag) {
                self.changes.push((tag, false));
            }
        }
    }

    /// One more source of each of `tags`, as given.
    pub fn add_all(&mut self, tags: &Tags) {
        for tag in tags.written() {
            self.add(tag);
        }
    }

    /// One fewer source of each of `tags`, as given.
    pub fn remove_all(&mut self, tags: &Tags) {
        for tag in tags.written() {
            self.remove(tag);
        }
    }

    /// The counts.
    #[must_use]
    pub fn counts(&self) -> &TagCounts {
        &self.counts
    }

    /// Whether something changed it that isn't told yet.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.changes.is_empty()
    }

    /// What came and went since last told, settled against what it was then:
    /// `true` for a tag now owned that wasn't, `false` for one no longer owned.
    /// A tag added and taken away again isn't told at all.
    fn take_changes(&mut self) -> Vec<(TagKey, bool)> {
        let mut seen: Vec<TagKey> = Vec::new();
        let mut settled = Vec::new();
        for (tag, came) in std::mem::take(&mut self.changes) {
            if seen.contains(&tag) {
                continue;
            }
            seen.push(tag);
            // Its first change says what it was: one that came wasn't
            // owned before.
            let was = !came;
            let now = self.counts.contains(tag);
            if was != now {
                settled.push((tag, now));
            }
        }
        settled
    }
}

impl Deref for OwnedTags {
    type Target = TagCounts;

    fn deref(&self) -> &TagCounts {
        &self.counts
    }
}

/// An actor came to own a tag: its first source, or the first of one under it.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct TagAdded {
    /// The actor.
    pub entity: Entity,
    /// The tag.
    pub tag: TagKey,
}

/// Triggered on an actor as it stops owning a tag, or the last one under it.
#[derive(EntityEvent, Debug, Clone, PartialEq, Eq)]
pub struct TagRemoved {
    /// The actor.
    pub entity: Entity,
    /// The tag.
    pub tag: TagKey,
}

/// Tells each tag an actor came to own or stopped owning, as events.
///
/// Triggering [`TagAdded`] and [`TagRemoved`]; the attribute plugin schedules
/// it.
pub fn publish_tags(
    mut commands: Commands,
    mut actors: Query<(Entity, &mut OwnedTags), Changed<OwnedTags>>,
) {
    for (entity, mut owned) in &mut actors {
        if !owned.is_pending() {
            continue;
        }
        for (tag, owns) in owned.bypass_change_detection().take_changes() {
            if owns {
                commands.trigger(TagAdded { entity, tag });
            } else {
                commands.trigger(TagRemoved { entity, tag });
            }
        }
    }
}

/// A guard: whether the agent owns `tag`, or a tag under it.
///
/// No for an agent without owned tags. A read-only system taking an agent and a
/// parameter, as `bevy_arbor` takes it.
#[must_use]
pub fn owns(In((agent, tag)): In<(Entity, TagKey)>, owned: Query<&OwnedTags>) -> bool {
    owned.get(agent).is_ok_and(|owned| owned.contains(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removing_a_tag_not_owned_tells_nothing() {
        let mut owned = OwnedTags::default();
        owned.remove("state.moving");
        assert!(!owned.is_pending());
        owned.add("immune.stun");
        owned.take_changes();
        owned.remove("immune.freeze");
        assert!(!owned.is_pending());
        assert!(owned.contains("immune"));
    }
}
