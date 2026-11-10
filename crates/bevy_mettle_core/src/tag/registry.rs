//! The tags there are, defined in content and code, checked and numbered.

use std::sync::Arc;

use bevy_app::App;
use bevy_ecs::resource::Resource;
use bevy_platform::collections::HashSet;

use super::{Tag, TagKey};
use crate::error::{Error, Errors};
use crate::registry::{AnyTag, Registry, TagId};

/// The tags there are, checked and numbered: a tree of paths, with no gaps.
///
/// Each tag is a path whose parent is a tag too, so they make a tree with no
/// gaps.
///
/// What data naming tags is checked against: the content's tags, and those
/// code adds ([`add_tag`](TagApp::add_tag)), with every tag above them, whether
/// content defines them or not. Each is numbered, a [`TagId`], in order of
/// name, so the same tags are numbered the same wherever they're checked: a
/// server's and its clients' alike. [`rebuild`](Self::rebuild) keeps the
/// numbers tags had. Read as a [`Registry`] of each tag's key: `get(id)`,
/// `name(id)`, `iter()`; [`id`](Self::id) takes a tag, by type or by name.
/// Cheap to clone; empty by default. A resource: the tags the app's content
/// was checked against.
#[derive(Resource, Debug, Clone, Default)]
pub struct TagRegistry {
    registry: Registry<AnyTag, TagKey>,
    /// The tags code adds, and every tag above them.
    code: Arc<Vec<TagKey>>,
    /// Every tag, in order of name.
    sorted: Arc<Vec<TagKey>>,
    /// Each tag's number by its key's index: a lookup is an index.
    by_key: Arc<Vec<Option<TagId>>>,
}

impl TagRegistry {
    /// Checks `definitions`, from however many files, and numbers them in
    /// order of name.
    ///
    /// # Errors
    ///
    /// Every tag defined twice, that isn't a path, or whose parent isn't a tag
    /// of its own.
    pub fn build(definitions: impl IntoIterator<Item = TagKey>) -> Result<Self, Errors> {
        Self::default().rebuild(definitions)
    }

    /// Checks `definitions` as [`build`](Self::build) does, with the tags code
    /// adds here, each tag here keeping its number: new ones numbered after
    /// the last, in order of name. What a reload does, so what holds a tag's
    /// number still names it.
    ///
    /// # Errors
    ///
    /// As `build`'s.
    pub fn rebuild(&self, definitions: impl IntoIterator<Item = TagKey>) -> Result<Self, Errors> {
        let mut errors = Vec::new();
        let mut known: HashSet<TagKey> = HashSet::default();
        for tag in definitions {
            if !known.insert(tag) {
                errors.push(Error::Duplicate {
                    kind: "tag",
                    name: tag.name().to_owned(),
                });
            }
        }
        known.extend(self.code.iter().copied());
        for &tag in &known {
            if !tag.is_path() {
                errors.push(Error::invalid(
                    format!("a tag called `{tag}`: a tag is words joined by `.`"),
                    None,
                ));
            } else if let Some(parent) = tag.parent()
                && !known.contains(&parent)
            {
                errors.push(unknown(parent, Some(tag.name())));
            }
        }
        if !errors.is_empty() {
            errors.sort_by_key(ToString::to_string);
            return Err(Errors(errors));
        }
        let mut sorted: Vec<TagKey> = known.into_iter().collect();
        sorted.sort_unstable_by_key(|tag| tag.name());
        let mut registry = self
            .registry
            .renumbered(sorted.iter().map(|tag| tag.name()));
        let mut by_key = Vec::new();
        for &tag in &sorted {
            if let Some(id) = registry.id(tag.name()) {
                registry.set(id, tag);
                if by_key.len() <= tag.index() {
                    by_key.resize(tag.index() + 1, None);
                }
                by_key[tag.index()] = Some(id);
            }
        }
        Ok(Self {
            registry,
            code: Arc::clone(&self.code),
            sorted: Arc::new(sorted),
            by_key: Arc::new(by_key),
        })
    }

    /// Adds `tag` and every tag above it as code's: in the registry as it's
    /// next built, whether content defines them or not.
    pub fn add_code(&mut self, tag: impl Into<TagKey>) {
        let code = Arc::make_mut(&mut self.code);
        for tag in tag.into().lineage() {
            if !code.contains(&tag) {
                code.push(tag);
            }
        }
    }

    /// The tags code adds, and every tag above them.
    pub fn code(&self) -> impl Iterator<Item = TagKey> + '_ {
        self.code.iter().copied()
    }

    /// Whether `tag` is defined.
    #[must_use]
    pub fn contains(&self, tag: impl Into<TagKey>) -> bool {
        self.id(tag).is_some()
    }

    /// `tag`'s number, if it's defined: a tag type, a key, or a name.
    #[must_use]
    pub fn id(&self, tag: impl Into<TagKey>) -> Option<TagId> {
        self.by_key.get(tag.into().index()).copied().flatten()
    }

    /// `T`'s number, if it's defined.
    #[must_use]
    pub fn id_of<T: Tag>(&self) -> Option<TagId> {
        self.id(T::key())
    }

    /// The tag numbered `id`, if there is one.
    #[must_use]
    pub fn key(&self, id: TagId) -> Option<TagKey> {
        self.registry.get(id).copied()
    }

    /// The number of `tag`, named within `within` (what names it) if it's
    /// named in something.
    ///
    /// # Errors
    ///
    /// If it isn't defined.
    pub fn resolve(&self, tag: TagKey, within: Option<&str>) -> Result<TagId, Error> {
        self.id(tag).ok_or_else(|| unknown(tag, within))
    }

    /// Whether every one of `tags` is defined, as [`resolve`](Self::resolve)
    /// says.
    ///
    /// # Errors
    ///
    /// The first that isn't.
    pub fn resolve_all(
        &self,
        tags: impl IntoIterator<Item = TagKey>,
        within: Option<&str>,
    ) -> Result<(), Error> {
        tags.into_iter()
            .try_for_each(|tag| self.resolve(tag, within).map(drop))
    }

    /// Every tag, in order of name.
    pub fn sorted(&self) -> impl Iterator<Item = TagKey> + '_ {
        self.sorted.iter().copied()
    }
}

/// That there's no tag called `tag`, named within `within`.
fn unknown(tag: TagKey, within: Option<&str>) -> Error {
    Error::Unknown {
        kind: "tag",
        name: tag.name().to_owned(),
        within: within.map(ToOwned::to_owned),
    }
}

/// Adding the tags code names to an app: `app.add_tag::<Stunned>()`.
pub trait TagApp {
    /// Adds `T`, and every tag above it, to the tags content is checked
    /// against.
    fn add_tag<T: Tag>(&mut self) -> &mut Self;
}

impl TagApp for App {
    fn add_tag<T: Tag>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<TagRegistry>()
            .add_code(T::key());
        self
    }
}

/// Read as the [`Registry`] of each tag's key, by number.
impl std::ops::Deref for TagRegistry {
    type Target = Registry<AnyTag, TagKey>;

    fn deref(&self) -> &Self::Target {
        &self.registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(names: &[&'static str]) -> Result<TagRegistry, String> {
        TagRegistry::build(names.iter().map(|&name| TagKey::named(name)))
            .map_err(|errors| errors.to_string())
    }

    #[test]
    fn a_tree_has_no_gaps() {
        assert!(registry(&["damage", "damage.fire"]).is_ok());
        assert_eq!(
            registry(&["damage.elemental.fire", "damage"]).unwrap_err(),
            "there's no tag called `damage.elemental` (in `damage.elemental.fire`)"
        );
        assert_eq!(
            registry(&["Fire"]).unwrap_err(),
            "a tag called `Fire`: a tag is words joined by `.`"
        );
    }

    #[test]
    fn the_fingerprint_ignores_order() {
        let a = registry(&["damage", "damage.fire", "state"]).unwrap();
        let b = registry(&["state", "damage.fire", "damage"]).unwrap();
        assert_eq!(a.fingerprint(), b.fingerprint());
        assert_ne!(
            a.fingerprint(),
            registry(&["damage"]).unwrap().fingerprint()
        );
    }

    #[test]
    fn tags_are_numbered_by_name_and_keep_their_numbers() {
        let first = registry(&["state", "damage", "damage.fire"]).unwrap();
        let damage = TagKey::named("damage");
        let state = TagKey::named("state");
        assert_eq!(first.id(damage), Some(TagId::new(0)));
        assert_eq!(first.id(state), Some(TagId::new(2)));
        assert_eq!(first.key(TagId::new(2)), Some(state));

        let fire = TagKey::named("damage.fire");
        let reloaded = first
            .rebuild(["damage", "damage.cold", "state"].map(TagKey::named))
            .unwrap();
        assert_eq!(reloaded.id(state), Some(TagId::new(2)), "kept");
        assert_eq!(
            reloaded.id("damage.cold"),
            Some(TagId::new(3)),
            "after the last"
        );
        assert_eq!(reloaded.id(fire), None);
        assert_eq!(reloaded.key(TagId::new(1)), None, "gone");
        assert_ne!(
            reloaded.fingerprint(),
            registry(&["damage", "damage.cold", "state"])
                .unwrap()
                .fingerprint()
        );
    }
}
