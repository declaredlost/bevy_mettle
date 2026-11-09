//! Tags held by count: what an actor owns.

use std::fmt;

use bevy_reflect::prelude::*;

use super::{TagKey, Tags};

/// Counts by tag, kept densely by key: a lookup is an index.
#[derive(Clone, Default, Reflect)]
#[reflect(Clone, Default)]
struct Counts(Vec<u32>);

impl Counts {
    fn get(&self, tag: TagKey) -> u32 {
        self.0.get(tag.index()).copied().unwrap_or_default()
    }

    fn add(&mut self, tag: TagKey) {
        let index = tag.index();
        if index >= self.0.len() {
            self.0.resize(index + 1, 0);
        }
        self.0[index] += 1;
    }

    /// One fewer of `tag`: whether there was one.
    fn take(&mut self, tag: TagKey) -> bool {
        match self.0.get_mut(tag.index()) {
            Some(count) if *count > 0 => {
                *count -= 1;
                true
            }
            _ => false,
        }
    }

    fn iter(&self) -> impl Iterator<Item = (TagKey, u32)> + '_ {
        (0u32..)
            .zip(&self.0)
            .filter(|(_, count)| **count > 0)
            .map(|(index, count)| (TagKey::at(index), *count))
    }
}

/// The same counts, however long either's kept.
impl PartialEq for Counts {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl Eq for Counts {}

/// Tags held by count, and every tag above them: what an actor owns.
///
/// While its states hold. Each source of a tag (an effect granting it, the game
/// marking the actor moving) adds one and takes its one away, so two effects
/// granting `immune.stun` keep it until both are gone.
#[derive(Clone, Default, PartialEq, Eq, Reflect)]
#[reflect(Clone, Default, PartialEq)]
pub struct TagCounts {
    /// How many hold each tag, given or above one given.
    counts: Counts,
    /// How many hold each tag as given: what can be taken away.
    given: Counts,
}

impl TagCounts {
    /// One more source of `tag`.
    pub fn add(&mut self, tag: impl Into<TagKey>) {
        let tag = tag.into();
        self.given.add(tag);
        for tag in tag.lineage() {
            self.counts.add(tag);
        }
    }

    /// One fewer source of `tag`: gone with its last.
    ///
    /// Nothing for a tag not given, even one held under another: `false` then.
    pub fn remove(&mut self, tag: impl Into<TagKey>) -> bool {
        let tag = tag.into();
        if !self.given.take(tag) {
            return false;
        }
        for tag in tag.lineage() {
            self.counts.take(tag);
        }
        true
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

    /// Whether anything holds `tag`, or a tag under it.
    #[must_use]
    pub fn contains(&self, tag: impl Into<TagKey>) -> bool {
        self.counts.get(tag.into()) > 0
    }

    /// How many hold `tag`, or tags under it.
    #[must_use]
    pub fn count(&self, tag: impl Into<TagKey>) -> u32 {
        self.counts.get(tag.into())
    }

    /// Whether anything holds any of `tags`.
    #[must_use]
    pub fn contains_any(&self, tags: &Tags) -> bool {
        tags.written().any(|tag| self.contains(tag))
    }

    /// Whether something holds every one of `tags`.
    #[must_use]
    pub fn contains_all(&self, tags: &Tags) -> bool {
        tags.written().all(|tag| self.contains(tag))
    }

    /// Every tag held, given or above, with how many hold it.
    pub fn iter(&self) -> impl Iterator<Item = (TagKey, u32)> + '_ {
        self.counts.iter()
    }
}

/// As the tags held, by name, with their counts.
impl fmt::Debug for TagCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map()
            .entries(self.iter().map(|(tag, count)| (tag.name(), count)))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_holds_until_its_last_source_goes() {
        let stun = TagKey::named("immune.stun");
        let mut owned = TagCounts::default();
        owned.add(stun);
        owned.add(stun);
        owned.remove(stun);
        assert!(owned.contains(stun));
        assert!(owned.contains("immune"));
        owned.remove(stun);
        assert!(!owned.contains(stun));
        assert!(!owned.contains("immune"));
    }

    #[test]
    fn a_parent_counts_every_source_under_it() {
        let mut owned = TagCounts::default();
        owned.add("immune.stun");
        owned.add("immune.freeze");
        assert_eq!(owned.count("immune"), 2);
    }

    #[test]
    fn removing_a_tag_not_given_changes_nothing() {
        let mut owned = TagCounts::default();
        owned.add("immune.stun");
        assert!(!owned.remove("immune.freeze"));
        assert!(!owned.remove("immune"));
        assert!(owned.contains("immune.stun"));
        assert_eq!(owned.count("immune"), 1);
    }

    #[test]
    fn counts_kept_longer_are_still_the_same() {
        let mut short = TagCounts::default();
        short.add("immune.stun");
        let mut long = short.clone();
        long.add("zzz.later");
        long.remove("zzz.later");
        assert_eq!(short, long);
    }
}
