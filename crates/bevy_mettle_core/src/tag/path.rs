//! Tags as paths in a tree.

use super::Tag;
use crate::Id;

impl Id<Tag> {
    /// The tag right above it: `damage.elemental` for `damage.elemental.fire`.
    /// `None` at the top.
    #[must_use]
    pub fn parent(&self) -> Option<Id<Tag>> {
        let (parent, _) = self.as_str().rsplit_once('.')?;
        Some(Id::new(parent))
    }

    /// It, then every tag above it, nearest first: `damage.elemental.fire`,
    /// `damage.elemental`, `damage`.
    pub fn lineage(&self) -> impl Iterator<Item = Id<Tag>> {
        std::iter::successors(Some(self.clone()), Id::parent)
    }

    /// Whether it's `tag` or under it: `damage.elemental.fire` is within
    /// `damage.elemental` and `damage`, not `damage.fire`. What has a tag has
    /// every tag above it, so a rule on `damage.elemental` holds for fire.
    #[must_use]
    pub fn is_within(&self, tag: &Id<Tag>) -> bool {
        let (it, above) = (self.as_str(), tag.as_str());
        it.strip_prefix(above)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('.'))
    }

    /// Whether it's a path: words of `a` to `z`, `0` to `9` and `_`, joined by
    /// `.`.
    pub(crate) fn is_path(&self) -> bool {
        self.as_str().split('.').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_is_within_itself_and_what_is_above_it() {
        let fire = Id::<Tag>::from_static("damage.elemental.fire");
        assert!(fire.is_within(&Id::from_static("damage.elemental.fire")));
        assert!(fire.is_within(&Id::from_static("damage.elemental")));
        assert!(fire.is_within(&Id::from_static("damage")));
        assert!(!fire.is_within(&Id::from_static("damage.elem")));
        assert!(!Id::<Tag>::from_static("damage").is_within(&fire));
        assert_eq!(fire.parent(), Some(Id::from_static("damage.elemental")));
        assert_eq!(Id::<Tag>::from_static("damage").parent(), None);
        let lineage: Vec<String> = fire.lineage().map(|tag| tag.to_string()).collect();
        assert_eq!(
            lineage,
            ["damage.elemental.fire", "damage.elemental", "damage"]
        );
    }

    #[test]
    fn a_path_is_words_joined_by_dots() {
        assert!(Id::<Tag>::from_static("delivery.attack.melee_2").is_path());
        for bad in ["", "a..b", ".a", "a.", "Fire", "a-b"] {
            assert!(!Id::<Tag>::new(bad).is_path(), "{bad}");
        }
    }
}
