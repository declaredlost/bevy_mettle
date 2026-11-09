//! Gameplay tags: labels in a tree, sets and counts of them, and owned tags.
//!
//! Code names them by type ([`Tag`]), data by name; both come to the same
//! [`TagKey`].

mod counts;
mod key;
mod owned;
mod registry;

use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use self::counts::TagCounts;
pub use self::key::TagKey;
pub use self::owned::{OwnedTags, TagAdded, TagRemoved, owns, publish_tags};
pub use self::registry::{TagApp, TagId, TagRegistry};

/// A gameplay tag: a label in a tree, as a marker type its place is on.
///
/// `damage.elemental.fire` is under `damage.elemental`, under `damage`; what
/// has a tag has every tag above it. What effects, abilities and hits are
/// labelled with, and what an actor owns while a state holds. Derive it on a
/// unit struct, under its parent:
///
/// ```ignore
/// #[derive(Tag)]
/// struct State;                 // "state"
///
/// #[derive(Tag)]
/// #[tag(parent = State)]
/// struct Stunned;               // "state.stunned"
/// ```
///
/// Its name is its type's in `snake_case` under its parent's, unless
/// `#[tag(name = "..")]` says. Code names it by its type, so a misspelled tag
/// doesn't compile: `owned.contains(Stunned)`. Data names it by name, and
/// both come to the same [`TagKey`]. Tags only data names needn't be types.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a tag",
    label = "not a tag",
    note = "derive it: `#[derive(Tag)]`"
)]
pub trait Tag: Send + Sync + 'static {
    /// Its own part of its name: `"stunned"`.
    const SEGMENT: &'static str;

    /// The tag it's under: [`Root`] for a top-level one.
    type Parent: Tag;

    /// Its key: made once, from its parent's key and its segment.
    fn key() -> TagKey;
}

pub use bevy_mettle_macros::Tag;

/// The root every tag hangs from: what a top-level tag is under.
///
/// It has no name, and is never owned or written in data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {}

impl Tag for Root {
    type Parent = Self;

    const SEGMENT: &'static str = "";

    fn key() -> TagKey {
        TagKey::ROOT
    }
}

/// A set of tags: what an effect, an ability or a hit is labelled with.
///
/// Has every tag above the ones it's given too, so asking it for
/// `effect.affliction` finds `effect.affliction.poison`. Written to data as
/// what it was given alone.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    /// The tags as given.
    written: BTreeSet<TagKey>,
    /// Those and every tag above them.
    all: BTreeSet<TagKey>,
}

impl Tags {
    /// No tags.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds `tag`, and every tag above it.
    pub fn insert(&mut self, tag: impl Into<TagKey>) {
        let tag = tag.into();
        self.all.extend(tag.lineage());
        self.written.insert(tag);
    }

    /// Takes `tag` out, as given, and what's above it unless another tag given
    /// is under it too.
    pub fn remove(&mut self, tag: impl Into<TagKey>) {
        if self.written.remove(&tag.into()) {
            self.all = self.written.iter().flat_map(|tag| tag.lineage()).collect();
        }
    }

    /// Whether it has `tag`: given, or above one given.
    #[must_use]
    pub fn contains(&self, tag: impl Into<TagKey>) -> bool {
        self.all.contains(&tag.into())
    }

    /// Whether it has any of `tags`.
    #[must_use]
    pub fn contains_any(&self, tags: &Tags) -> bool {
        tags.written.iter().any(|tag| self.contains(*tag))
    }

    /// Whether it has every one of `tags`.
    #[must_use]
    pub fn contains_all(&self, tags: &Tags) -> bool {
        tags.written.iter().all(|tag| self.contains(*tag))
    }

    /// The tags as given, without those above them.
    pub fn written(&self) -> impl Iterator<Item = TagKey> {
        self.written.iter().copied()
    }

    /// Every tag it has: given, and above.
    pub fn iter(&self) -> impl Iterator<Item = TagKey> {
        self.all.iter().copied()
    }

    /// Whether it has none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.written.is_empty()
    }
}

impl<K: Into<TagKey>> FromIterator<K> for Tags {
    fn from_iter<I: IntoIterator<Item = K>>(tags: I) -> Self {
        let mut set = Self::new();
        set.extend(tags);
        set
    }
}

impl<K: Into<TagKey>> Extend<K> for Tags {
    fn extend<I: IntoIterator<Item = K>>(&mut self, tags: I) {
        for tag in tags {
            self.insert(tag);
        }
    }
}

/// As the names given, in order of name: the same wherever it's written.
impl Serialize for Tags {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut names: Vec<&str> = self.written.iter().map(|tag| tag.name()).collect();
        names.sort_unstable();
        names.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Tags {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Vec::<TagKey>::deserialize(deserializer)?
            .into_iter()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(names: &[&'static str]) -> Tags {
        names.iter().copied().collect()
    }

    #[test]
    fn it_has_every_tag_above_what_its_given() {
        let poisoned = tags(&["effect.affliction.poison"]);
        for tag in ["effect.affliction.poison", "effect.affliction", "effect"] {
            assert!(poisoned.contains(tag), "{tag}");
        }
        assert!(!poisoned.contains("effect.hazard"));
        assert_eq!(poisoned.written().count(), 1);
    }

    #[test]
    fn removing_a_tag_keeps_what_another_is_under() {
        let mut both = tags(&["effect.affliction.poison", "effect.affliction.bleed"]);
        both.remove("effect.affliction.poison");
        assert!(both.contains("effect.affliction"));
        assert!(!both.contains("effect.affliction.poison"));
    }

    #[test]
    fn it_reads_and_writes_what_its_given() {
        use serde_test::{Token, assert_tokens};

        let poisoned: Tags = ["effect.affliction.poison"].into_iter().collect();
        assert_eq!(poisoned.iter().count(), 3);
        assert_tokens(
            &poisoned,
            &[
                Token::Seq { len: Some(1) },
                Token::Str("effect.affliction.poison"),
                Token::SeqEnd,
            ],
        );
    }
}
