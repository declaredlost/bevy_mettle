//! Tags held by count: what an actor owns.

use std::collections::BTreeMap;

use super::{Tag, Tags};
use crate::Id;

/// Tags held by count, and every tag above them: what an actor owns while
/// its states hold. Each source of a tag (an effect granting it, the game
/// marking the actor moving) adds one and takes its one away, so two
/// effects granting `immune.stun` keep it until both are gone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TagCounts {
    /// How many hold each tag, given or above one given.
    counts: BTreeMap<Id<Tag>, u32>,
}

impl TagCounts {
    /// One more source of `tag`.
    pub fn add(&mut self, tag: &Id<Tag>) {
        for tag in tag.lineage() {
            *self.counts.entry(tag).or_default() += 1;
        }
    }

    /// One fewer source of `tag`: gone with its last.
    pub fn remove(&mut self, tag: &Id<Tag>) {
        for tag in tag.lineage() {
            if let Some(count) = self.counts.get_mut(&tag) {
                *count -= 1;
                if *count == 0 {
                    self.counts.remove(&tag);
                }
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

    /// Whether anything holds `tag`, or a tag under it.
    #[must_use]
    pub fn contains(&self, tag: &Id<Tag>) -> bool {
        self.counts.contains_key(tag)
    }

    /// How many hold `tag`, or tags under it.
    #[must_use]
    pub fn count(&self, tag: &Id<Tag>) -> u32 {
        self.counts.get(tag).copied().unwrap_or_default()
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
    pub fn iter(&self) -> impl Iterator<Item = (&Id<Tag>, u32)> {
        self.counts.iter().map(|(tag, count)| (tag, *count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_holds_until_its_last_source_goes() {
        let stun = Id::<Tag>::from_static("immune.stun");
        let mut owned = TagCounts::default();
        owned.add(&stun);
        owned.add(&stun);
        owned.remove(&stun);
        assert!(owned.contains(&stun));
        assert!(owned.contains(&Id::from_static("immune")));
        owned.remove(&stun);
        assert!(!owned.contains(&stun));
        assert!(!owned.contains(&Id::from_static("immune")));
    }

    #[test]
    fn a_parent_counts_every_source_under_it() {
        let mut owned = TagCounts::default();
        owned.add(&Id::from_static("immune.stun"));
        owned.add(&Id::from_static("immune.freeze"));
        assert_eq!(owned.count(&Id::from_static("immune")), 2);
    }
}
