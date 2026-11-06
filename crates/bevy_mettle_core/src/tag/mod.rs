//! Gameplay tags: labels in a tree, defined in data. Sets of them, counts
//! of them, and the tree they're checked against.

mod counts;
mod owned;
mod path;
mod tree;

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use self::counts::TagCounts;
pub use self::owned::{OwnedTags, TagAdded, TagRemoved, owns, publish_tags};
pub use self::tree::TagTree;
use crate::{Id, IdTarget};

/// A gameplay tag: a label in a tree, defined in data, its id its path.
/// `damage.elemental.fire` is under `damage.elemental`, under `damage`;
/// what has a tag has every tag above it. What effects, abilities and hits
/// are labelled with, and what an actor owns while a state holds
/// (`state.moving`, `immune.stun`). Named by an `Id<Tag>`; defined in data,
/// so a mod can add its own:
///
/// ```ron
/// [(id: "damage"), (id: "damage.elemental"), (id: "damage.elemental.fire")]
/// ```
#[derive(IdTarget)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tag {
    /// Its id: its path in the tree.
    pub id: Id<Tag>,
}

/// A set of tags: what an effect, an ability or a hit is labelled with.
/// Has every tag above the ones it's given too, so asking it for
/// `effect.affliction` finds `effect.affliction.poison`. Written to data as
/// what it was given alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    /// The tags as given.
    written: BTreeSet<Id<Tag>>,
    /// Those and every tag above them.
    all: BTreeSet<Id<Tag>>,
}

impl Tags {
    /// No tags.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `tag`, and every tag above it.
    pub fn insert(&mut self, tag: Id<Tag>) {
        self.all.extend(tag.lineage());
        self.written.insert(tag);
    }

    /// Takes `tag` out, as given, and what's above it unless another tag
    /// given is under it too.
    pub fn remove(&mut self, tag: &Id<Tag>) {
        if self.written.remove(tag) {
            self.all = self.written.iter().flat_map(Id::lineage).collect();
        }
    }

    /// Whether it has `tag`: given, or above one given.
    #[must_use]
    pub fn contains(&self, tag: &Id<Tag>) -> bool {
        self.all.contains(tag)
    }

    /// Whether it has any of `tags`.
    #[must_use]
    pub fn contains_any(&self, tags: &Tags) -> bool {
        tags.written.iter().any(|tag| self.contains(tag))
    }

    /// Whether it has every one of `tags`.
    #[must_use]
    pub fn contains_all(&self, tags: &Tags) -> bool {
        tags.written.iter().all(|tag| self.contains(tag))
    }

    /// The tags as given, without those above them.
    pub fn written(&self) -> impl Iterator<Item = &Id<Tag>> {
        self.written.iter()
    }

    /// Every tag it has: given, and above.
    pub fn iter(&self) -> impl Iterator<Item = &Id<Tag>> {
        self.all.iter()
    }

    /// Whether it has none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.written.is_empty()
    }
}

impl FromIterator<Id<Tag>> for Tags {
    fn from_iter<I: IntoIterator<Item = Id<Tag>>>(tags: I) -> Self {
        let mut set = Self::new();
        set.extend(tags);
        set
    }
}

impl Extend<Id<Tag>> for Tags {
    fn extend<I: IntoIterator<Item = Id<Tag>>>(&mut self, tags: I) {
        for tag in tags {
            self.insert(tag);
        }
    }
}

impl Serialize for Tags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.written.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Tags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(BTreeSet::<Id<Tag>>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(ids: &[&'static str]) -> Tags {
        ids.iter().map(|&id| Id::from_static(id)).collect()
    }

    #[test]
    fn it_has_every_tag_above_what_its_given() {
        let poisoned = tags(&["effect.affliction.poison"]);
        for tag in ["effect.affliction.poison", "effect.affliction", "effect"] {
            assert!(poisoned.contains(&Id::from_static(tag)), "{tag}");
        }
        assert!(!poisoned.contains(&Id::from_static("effect.hazard")));
        assert_eq!(poisoned.written().count(), 1);
    }

    #[test]
    fn removing_a_tag_keeps_what_another_is_under() {
        let mut both = tags(&["effect.affliction.poison", "effect.affliction.bleed"]);
        both.remove(&Id::from_static("effect.affliction.poison"));
        assert!(both.contains(&Id::from_static("effect.affliction")));
        assert!(!both.contains(&Id::from_static("effect.affliction.poison")));
    }

    #[test]
    fn it_reads_and_writes_what_its_given() {
        let poisoned: Tags = ron::from_str(r#"["effect.affliction.poison"]"#).unwrap();
        assert_eq!(poisoned.iter().count(), 3);
        assert_eq!(
            ron::to_string(&poisoned).unwrap(),
            r#"["effect.affliction.poison"]"#
        );
    }
}
