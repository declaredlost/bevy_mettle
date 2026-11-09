//! What every registry keeps: entries by number, and their numbers by name and
//! by type.

use std::any::TypeId;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use bevy_platform::collections::HashMap;
use fnv::FnvHasher;
use smol_str::SmolStr;

use super::Numbering;

/// A number a registry gives what it holds: what `number!` defines.
pub trait Number: Copy + Eq + Hash + fmt::Debug + Send + Sync + 'static {
    /// The one numbered `index`.
    fn from_index(index: u32) -> Self;

    /// Its index: where it's kept.
    fn to_index(self) -> usize;
}

/// Entries by number, `I`, and their numbers by name and by type: what every
/// registry keeps, as Bevy's `Components` keeps each component's info by
/// `ComponentId`.
///
/// Filled one of two ways:
///
/// - **By code**, as an app is built: [`add`](Self::add) numbers each entry
///   after the last, by the type that adds it (attributes, behaviours,
///   executions).
/// - **From content**, as it's checked: [`renumbered`](Self::renumbered)
///   numbers names in the order given (sorted, so a server and its clients
///   number the same content the same), each name here keeping its number, then
///   [`set`](Self::set) fills each in. Types code names
///   ([`name_type`](Self::name_type)) are kept, and resolved to the numbers
///   content gives their names.
///
/// Cheap to clone.
pub struct Registry<I, T> {
    numbering: Arc<Numbering<SmolStr>>,
    entries: Arc<Vec<Option<T>>>,
    /// The types code names, with their names.
    named: Arc<Vec<(TypeId, &'static str)>>,
    by_type: Arc<HashMap<TypeId, I>>,
    /// The name of the type that added each entry, for an error.
    added_by: Arc<HashMap<TypeId, &'static str>>,
}

impl<I, T> Clone for Registry<I, T> {
    fn clone(&self) -> Self {
        Self {
            numbering: Arc::clone(&self.numbering),
            entries: Arc::clone(&self.entries),
            named: Arc::clone(&self.named),
            by_type: Arc::clone(&self.by_type),
            added_by: Arc::clone(&self.added_by),
        }
    }
}

impl<I, T> Default for Registry<I, T> {
    fn default() -> Self {
        Self {
            numbering: Arc::default(),
            entries: Arc::default(),
            named: Arc::default(),
            by_type: Arc::default(),
            added_by: Arc::default(),
        }
    }
}

/// As its names, by number.
impl<I, T> fmt::Debug for Registry<I, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.numbering.iter().map(|(_, name)| name))
            .finish()
    }
}

impl<I: Number, T: Clone> Registry<I, T> {
    /// The entry numbered `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: I) -> Option<&T> {
        self.entries.get(id.to_index())?.as_ref()
    }

    /// The number of what's called `name`, if anything is.
    #[must_use]
    pub fn id(&self, name: &str) -> Option<I> {
        self.numbering.number(name).map(I::from_index)
    }

    /// The number of what the type `K` names or adds, if it's in.
    #[must_use]
    pub fn id_of<K: 'static>(&self) -> Option<I> {
        self.id_of_type(TypeId::of::<K>())
    }

    /// The number of what the type `type_id` names or adds, if it's in.
    #[must_use]
    pub fn id_of_type(&self, type_id: TypeId) -> Option<I> {
        self.by_type.get(&type_id).copied()
    }

    /// The name of what's numbered `id`, if anything is.
    #[must_use]
    pub fn name(&self, id: I) -> Option<&str> {
        u32::try_from(id.to_index())
            .ok()
            .and_then(|number| self.numbering.name(number))
            .map(SmolStr::as_str)
    }

    /// Every entry with its number, in order of number.
    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        (0u32..)
            .zip(self.entries.iter())
            .filter_map(|(number, entry)| Some((I::from_index(number), entry.as_ref()?)))
    }

    /// Every entry with its number, in order of number, to change.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (I, &mut T)> {
        (0u32..)
            .zip(Arc::make_mut(&mut self.entries).iter_mut())
            .filter_map(|(number, entry)| Some((I::from_index(number), entry.as_mut()?)))
    }

    /// A hash of the names and their numbers, the same wherever they're
    /// numbered the same: for a client to check it numbers content as its
    /// server does. FNV-1a over each number's name and its length,
    /// little-endian; an empty number's length is `u64::MAX`.
    #[must_use]
    pub fn fingerprint(&self) -> u64 {
        let mut hash = FnvHasher::default();
        for number in (0u32..).take(self.len()) {
            match self.numbering.name(number) {
                Some(name) => {
                    hash.write(&(name.len() as u64).to_le_bytes());
                    hash.write(name.as_bytes());
                }
                None => hash.write(&u64::MAX.to_le_bytes()),
            }
        }
        hash.finish()
    }

    /// How many numbers there are, empty ones too: one past the last.
    #[must_use]
    pub fn len(&self) -> usize {
        self.numbering.len()
    }

    /// Whether nothing's numbered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.numbering.is_empty()
    }

    /// Adds `entry`, by the type `K` called `name`, numbered after the last;
    /// or replaces what `K` added: its number.
    ///
    /// # Panics
    ///
    /// If another type added something called `name`: data couldn't tell them
    /// apart.
    pub fn add<K: 'static>(&mut self, name: &'static str, entry: T) -> I {
        let type_id = TypeId::of::<K>();
        if let Some(known) = self.id_of_type(type_id) {
            Arc::make_mut(&mut self.entries)[known.to_index()] = Some(entry);
            return known;
        }
        if self.id(name).is_some() {
            let other = self
                .by_type
                .iter()
                .find(|(_, id)| self.name(**id) == Some(name))
                .and_then(|(other, _)| self.added_by.get(other))
                .copied()
                .unwrap_or("another type");
            panic!(
                "`{other}` and `{}` are both named `{name}`",
                std::any::type_name::<K>()
            );
        }
        let id = I::from_index(Arc::make_mut(&mut self.numbering).push(SmolStr::new_static(name)));
        let entries = Arc::make_mut(&mut self.entries);
        if entries.len() <= id.to_index() {
            entries.resize(id.to_index() + 1, None);
        }
        entries[id.to_index()] = Some(entry);
        Arc::make_mut(&mut self.by_type).insert(type_id, id);
        Arc::make_mut(&mut self.added_by).insert(type_id, std::any::type_name::<K>());
        id
    }

    /// Notes that code names what content calls `name` by the type `type_id`:
    /// resolved to its number as content's numbered.
    pub fn name_type(&mut self, type_id: TypeId, name: &'static str) {
        if self.named.iter().any(|(named, _)| *named == type_id) {
            return;
        }
        Arc::make_mut(&mut self.named).push((type_id, name));
        if let Some(id) = self.id(name) {
            Arc::make_mut(&mut self.by_type).insert(type_id, id);
        }
    }

    /// Numbered for content naming `names`, in the order given: each name here
    /// keeps its number, new ones are numbered after the last, and those gone
    /// leave their numbers empty. Every entry's empty, for
    /// [`set`](Self::set) to fill; the types code names are kept, and resolved.
    #[must_use]
    pub fn renumbered(&self, names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let numbering = self
            .numbering
            .renumbered(names.into_iter().map(|name| SmolStr::new(name.as_ref())));
        let by_type = self
            .named
            .iter()
            .filter_map(|&(type_id, name)| Some((type_id, I::from_index(numbering.number(name)?))))
            .collect();
        Self {
            entries: Arc::new(vec![None; numbering.len()]),
            numbering: Arc::new(numbering),
            named: Arc::clone(&self.named),
            by_type: Arc::new(by_type),
            added_by: Arc::clone(&self.added_by),
        }
    }

    /// Puts `entry` at `id`.
    ///
    /// # Panics
    ///
    /// If nothing's numbered `id`.
    pub fn set(&mut self, id: I, entry: T) {
        Arc::make_mut(&mut self.entries)[id.to_index()] = Some(entry);
    }

    /// The names code names by type that nothing's numbered: what content
    /// doesn't define.
    pub fn unnumbered(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.named
            .iter()
            .filter(|(type_id, _)| !self.by_type.contains_key(type_id))
            .map(|&(_, name)| name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::number! {
        struct ThingId;
    }

    struct Sword;
    struct Shield;

    #[test]
    fn code_adds_in_order_and_replaces_by_type() {
        let mut registry = Registry::<ThingId, u8>::default();
        let sword = registry.add::<Sword>("sword", 1);
        let shield = registry.add::<Shield>("shield", 2);
        assert_eq!((sword.index(), shield.index()), (0, 1));
        assert_eq!(registry.add::<Sword>("sword", 3), sword);
        assert_eq!(registry.get(sword), Some(&3));
        assert_eq!(registry.id("shield"), Some(shield));
        assert_eq!(registry.id_of::<Shield>(), Some(shield));
        assert_eq!(registry.name(sword), Some("sword"));
    }

    #[test]
    #[should_panic(expected = "are both named `sword`")]
    fn two_types_cant_share_a_name() {
        let mut registry = Registry::<ThingId, u8>::default();
        registry.add::<Sword>("sword", 1);
        registry.add::<Shield>("sword", 2);
    }

    #[test]
    fn content_keeps_its_numbers_and_resolves_the_types_code_names() {
        let mut registry = Registry::<ThingId, u8>::default();
        registry.name_type(TypeId::of::<Shield>(), "shield");
        let first = registry.renumbered(["shield", "sword"]);
        assert_eq!(first.id_of::<Shield>(), Some(ThingId::new(0)));
        assert_eq!(first.unnumbered().count(), 0);

        let mut reloaded = first.renumbered(["axe", "sword"]);
        assert_eq!(reloaded.id("sword"), Some(ThingId::new(1)), "kept");
        assert_eq!(reloaded.id("axe"), Some(ThingId::new(2)), "after the last");
        assert_eq!(reloaded.id_of::<Shield>(), None);
        assert_eq!(reloaded.unnumbered().collect::<Vec<_>>(), ["shield"]);
        reloaded.set(ThingId::new(2), 7);
        assert_eq!(reloaded.get(ThingId::new(2)), Some(&7));
        assert_eq!(reloaded.get(ThingId::new(0)), None, "gone");
    }
}
