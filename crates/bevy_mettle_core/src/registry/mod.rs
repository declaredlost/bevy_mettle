//! Registries: entries of each kind by number, and numbers by name and type.
//!
//! What an app has of each kind, by number, and its numbers by name and by
//! type; and content's kinds, with their keys, the names content writes, and
//! their ids, what content is numbered by, shared over a network.

use std::any::TypeId;
use std::fmt;
use std::hash::Hasher;
use std::sync::Arc;

use bevy_platform::collections::HashMap;
use fnv::FnvHasher;
use smol_str::SmolStr;

pub use self::id::{AbilityId, AttributeId, CurveId, EffectId, Id, TagId};
pub use self::key::Key;
pub use self::kind::{AnyAbility, AnyAttribute, AnyCurve, AnyEffect, AnyTag, Kind};
use self::numbering::Numbering;

mod id;
mod key;
mod kind;
mod numbering;

/// Entries of kind `K` by [`Id<K>`](Id), and their numbers by name and type.
///
/// What every registry keeps, as Bevy's `Components` keeps each component's
/// info by `ComponentId`.
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
pub struct Registry<K: Kind, T> {
    numbering: Arc<Numbering<SmolStr>>,
    entries: Arc<Vec<Option<T>>>,
    /// The types code names, with their names.
    named: Arc<Vec<(TypeId, &'static str)>>,
    by_type: Arc<HashMap<TypeId, Id<K>>>,
    /// The name of the type that added each entry, for an error.
    added_by: Arc<HashMap<TypeId, &'static str>>,
}

impl<K: Kind, T> Clone for Registry<K, T> {
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

impl<K: Kind, T> Default for Registry<K, T> {
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
impl<K: Kind, T> fmt::Debug for Registry<K, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.numbering.iter().map(|(_, name)| name))
            .finish()
    }
}

impl<K: Kind, T: Clone> Registry<K, T> {
    /// The entry numbered `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: Id<K>) -> Option<&T> {
        self.entries.get(id.index())?.as_ref()
    }

    /// The number of what's called `name`, if anything is.
    #[must_use]
    pub fn id(&self, name: &str) -> Option<Id<K>> {
        self.numbering.number(name).map(Id::new)
    }

    /// The number of what the type `N` names or adds, if it's in.
    #[must_use]
    pub fn id_of<N: 'static>(&self) -> Option<Id<K>> {
        self.id_of_type(TypeId::of::<N>())
    }

    /// The number of what the type `type_id` names or adds, if it's in.
    #[must_use]
    pub fn id_of_type(&self, type_id: TypeId) -> Option<Id<K>> {
        self.by_type.get(&type_id).copied()
    }

    /// The name of what's numbered `id`, if anything is.
    #[must_use]
    pub fn name(&self, id: Id<K>) -> Option<&str> {
        u32::try_from(id.index())
            .ok()
            .and_then(|number| self.numbering.name(number))
            .map(SmolStr::as_str)
    }

    /// Every entry with its number, in order of number.
    pub fn iter(&self) -> impl Iterator<Item = (Id<K>, &T)> {
        (0u32..)
            .zip(self.entries.iter())
            .filter_map(|(number, entry)| Some((Id::new(number), entry.as_ref()?)))
    }

    /// Every entry with its number, in order of number, to change.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Id<K>, &mut T)> {
        (0u32..)
            .zip(Arc::make_mut(&mut self.entries).iter_mut())
            .filter_map(|(number, entry)| Some((Id::new(number), entry.as_mut()?)))
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

    /// Adds `entry`, by the type `N` called `name`, numbered after the last;
    /// or replaces what `N` added: its number.
    ///
    /// # Panics
    ///
    /// If another type added something called `name`: data couldn't tell them
    /// apart.
    pub fn add<N: 'static>(&mut self, name: &'static str, entry: T) -> Id<K> {
        self.add_type(TypeId::of::<N>(), std::any::type_name::<N>(), name, entry)
    }

    /// Adds `entry`, by the type `type_id` (its Rust name `type_name`) called
    /// `name`, numbered after the last; or replaces what that type added: its
    /// number. For a type known only at runtime, as reflection knows it.
    ///
    /// # Panics
    ///
    /// If another type added something called `name`.
    pub fn add_type(
        &mut self,
        type_id: TypeId,
        type_name: &'static str,
        name: &'static str,
        entry: T,
    ) -> Id<K> {
        if let Some(known) = self.id_of_type(type_id) {
            Arc::make_mut(&mut self.entries)[known.index()] = Some(entry);
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
            panic!("`{other}` and `{type_name}` are both named `{name}`");
        }
        let id = Id::new(Arc::make_mut(&mut self.numbering).push(SmolStr::new_static(name)));
        let entries = Arc::make_mut(&mut self.entries);
        if entries.len() <= id.index() {
            entries.resize(id.index() + 1, None);
        }
        entries[id.index()] = Some(entry);
        Arc::make_mut(&mut self.by_type).insert(type_id, id);
        Arc::make_mut(&mut self.added_by).insert(type_id, type_name);
        id
    }

    /// The same, numbered in order of name: as content is, so the same
    /// entries are numbered the same wherever they're added, in whatever
    /// order. Each keeps its type; names numbered but empty are let go.
    #[must_use]
    pub fn sorted(&self) -> Self {
        let mut named: Vec<(SmolStr, Option<T>)> = self
            .numbering
            .iter()
            .map(|(number, name)| {
                let entry = self.entries.get(number as usize).cloned().flatten();
                (name.clone(), entry)
            })
            .collect();
        named.sort_by(|(a, _), (b, _)| a.cmp(b));
        let mut numbering = Numbering::default();
        let mut entries = Vec::with_capacity(named.len());
        for (name, entry) in named {
            numbering.push(name);
            entries.push(entry);
        }
        let by_type = self
            .by_type
            .iter()
            .filter_map(|(&type_id, &id)| {
                let name = self.name(id)?;
                Some((type_id, Id::new(numbering.number(name)?)))
            })
            .collect();
        Self {
            numbering: Arc::new(numbering),
            entries: Arc::new(entries),
            named: Arc::clone(&self.named),
            by_type: Arc::new(by_type),
            added_by: Arc::clone(&self.added_by),
        }
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
            .filter_map(|&(type_id, name)| Some((type_id, Id::new(numbering.number(name)?))))
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
    pub fn set(&mut self, id: Id<K>, entry: T) {
        Arc::make_mut(&mut self.entries)[id.index()] = Some(entry);
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

    #[derive(crate::registry::Kind)]
    enum AnyThing {}

    type ThingId = Id<AnyThing>;

    struct Sword;
    struct Shield;

    #[test]
    fn code_adds_in_order_and_replaces_by_type() {
        let mut registry = Registry::<AnyThing, u8>::default();
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
    fn sorted_numbers_by_name_whatever_order_they_were_added_in() {
        let mut one = Registry::<AnyThing, u8>::default();
        one.add::<Sword>("sword", 1);
        one.add::<Shield>("shield", 2);
        let mut other = Registry::<AnyThing, u8>::default();
        other.add::<Shield>("shield", 2);
        other.add::<Sword>("sword", 1);
        let (one, other) = (one.sorted(), other.sorted());
        for registry in [&one, &other] {
            let shield = registry.id_of::<Shield>().unwrap();
            let sword = registry.id_of::<Sword>().unwrap();
            assert_eq!((shield.index(), sword.index()), (0, 1), "in order of name");
            assert_eq!(registry.get(sword), Some(&1), "each keeps its entry");
            assert_eq!(registry.id("shield"), Some(shield));
        }
        assert_eq!(one.fingerprint(), other.fingerprint());
    }

    #[test]
    #[should_panic(expected = "are both named `sword`")]
    fn two_types_cant_share_a_name() {
        let mut registry = Registry::<AnyThing, u8>::default();
        registry.add::<Sword>("sword", 1);
        registry.add::<Shield>("sword", 2);
    }

    #[test]
    fn content_keeps_its_numbers_and_resolves_the_types_code_names() {
        let mut registry = Registry::<AnyThing, u8>::default();
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
