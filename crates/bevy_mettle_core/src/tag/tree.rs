//! The tags there are, defined in data and checked.

use std::collections::BTreeSet;
use std::hash::Hasher;
use std::sync::Arc;

use fnv::FnvHasher;

use super::Tag;
use crate::{Id, MettleError, MettleErrors};

/// The tags there are, checked: each a path whose parent is a tag too.
///
/// So the tree has no gaps. What data naming tags is checked against. Cheap to
/// clone; empty by default.
#[derive(Debug, Clone, Default)]
pub struct TagTree(Arc<BTreeSet<Id<Tag>>>);

impl TagTree {
    /// Checks `definitions`, from however many files.
    ///
    /// # Errors
    ///
    /// Every tag defined twice, that isn't a path, or whose parent isn't a tag
    /// of its own.
    pub fn build<'a>(definitions: impl IntoIterator<Item = &'a Tag>) -> Result<Self, MettleErrors> {
        let mut errors = Vec::new();
        let mut tags = BTreeSet::new();
        for def in definitions {
            if !tags.insert(def.id.clone()) {
                errors.push(MettleError::Duplicate {
                    kind: "tag",
                    id: def.id.to_string(),
                });
            }
        }
        for tag in &tags {
            if !tag.is_path() {
                errors.push(MettleError::Invalid {
                    what: format!("a tag called `{tag}`: a tag is words joined by `.`"),
                    within: None,
                });
            } else if let Some(parent) = tag.parent()
                && !tags.contains(&parent)
            {
                errors.push(MettleError::unknown::<Tag>(
                    parent.as_str(),
                    Some(tag.as_str()),
                ));
            }
        }
        if errors.is_empty() {
            Ok(Self(Arc::new(tags)))
        } else {
            Err(MettleErrors(errors))
        }
    }

    /// Whether `tag` is defined.
    #[must_use]
    pub fn contains(&self, tag: &Id<Tag>) -> bool {
        self.0.contains(tag)
    }

    /// `tag`, if it's defined, named within `within` (what names it) if it's
    /// named in something.
    ///
    /// # Errors
    ///
    /// If it isn't.
    pub fn known(&self, tag: &Id<Tag>, within: Option<&str>) -> Result<(), MettleError> {
        if self.contains(tag) {
            Ok(())
        } else {
            Err(MettleError::unknown::<Tag>(tag.as_str(), within))
        }
    }

    /// Every one of `tags`, if they're defined.
    ///
    /// # Errors
    ///
    /// The first that isn't.
    pub fn known_all<'a>(
        &self,
        tags: impl IntoIterator<Item = &'a Id<Tag>>,
        within: Option<&str>,
    ) -> Result<(), MettleError> {
        tags.into_iter().try_for_each(|tag| self.known(tag, within))
    }

    /// Every tag, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Id<Tag>> {
        self.0.iter()
    }

    /// A hash of the tags, the same wherever they're the same whatever order
    /// they came in: for a client to check it has its server's. FNV-1a over
    /// each path and its length, little-endian.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = FnvHasher::default();
        for tag in self.iter() {
            hash.write(&(tag.as_str().len() as u64).to_le_bytes());
            hash.write(tag.as_str().as_bytes());
        }
        hash.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(ids: &[&'static str]) -> Result<TagTree, String> {
        let defs: Vec<Tag> = ids
            .iter()
            .map(|&id| Tag {
                id: Id::from_static(id),
            })
            .collect();
        TagTree::build(&defs).map_err(|errors| errors.to_string())
    }

    #[test]
    fn a_tree_has_no_gaps() {
        assert!(tree(&["damage", "damage.fire"]).is_ok());
        assert_eq!(
            tree(&["damage.elemental.fire", "damage"]).unwrap_err(),
            "there's no tag called `damage.elemental` (in `damage.elemental.fire`)"
        );
        assert_eq!(
            tree(&["Fire"]).unwrap_err(),
            "a tag called `Fire`: a tag is words joined by `.`"
        );
    }

    #[test]
    fn the_fingerprint_ignores_order() {
        let a = tree(&["damage", "damage.fire", "state"]).unwrap();
        let b = tree(&["state", "damage.fire", "damage"]).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(a.fingerprint(), tree(&["damage"]).unwrap().fingerprint());
    }
}
