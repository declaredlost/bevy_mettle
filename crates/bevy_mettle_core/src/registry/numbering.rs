//! Numbering what content names: each name its number, kept as content is
//! reloaded.

use std::borrow::Borrow;
use std::hash::Hash;

use bevy_platform::collections::HashMap;

/// The numbers of what a registry holds, by name.
///
/// What a registry keeps beside its entries, as Bevy's `Components` keeps
/// each component's `ComponentId`. Numbered in the order names are given, so
/// content given in the same order is numbered the same wherever it's
/// loaded: a server's and its clients' alike. Callers give names sorted, so
/// the order files load in doesn't matter.
///
/// Renumbered as content is reloaded, a name keeps its number: what holds a
/// number (an active effect, a replica) still names the same thing. New names
/// are numbered after the last; a name gone leaves its number empty.
#[derive(Debug, Clone)]
pub(crate) struct Numbering<K> {
    names: Vec<Option<K>>,
    numbers: HashMap<K, u32>,
}

impl<K> Default for Numbering<K> {
    fn default() -> Self {
        Self {
            names: Vec::new(),
            numbers: HashMap::default(),
        }
    }
}

impl<K: Hash + Eq + Clone> Numbering<K> {
    /// `names` numbered: as they are here if they're here, new ones after the
    /// last in the order given. Here and not in `names`, a number is left
    /// empty. A name given twice is numbered once.
    ///
    /// # Panics
    ///
    /// Past 2^32 numbers.
    #[must_use]
    pub(crate) fn renumbered(&self, names: impl IntoIterator<Item = K>) -> Self {
        let mut next = Self {
            names: vec![None; self.names.len()],
            numbers: HashMap::default(),
        };
        for name in names {
            if next.numbers.contains_key(&name) {
                continue;
            }
            let number = if let Some(&number) = self.numbers.get(&name) {
                number
            } else {
                next.names.push(None);
                u32::try_from(next.names.len() - 1).expect("fewer than 2^32 names")
            };
            next.names[number as usize] = Some(name.clone());
            next.numbers.insert(name, number);
        }
        next
    }

    /// `name` numbered after the last, or its number if it has one.
    ///
    /// # Panics
    ///
    /// Past 2^32 numbers.
    pub(crate) fn push(&mut self, name: K) -> u32 {
        if let Some(&number) = self.numbers.get(&name) {
            return number;
        }
        let number = u32::try_from(self.names.len()).expect("fewer than 2^32 names");
        self.names.push(Some(name.clone()));
        self.numbers.insert(name, number);
        number
    }

    /// `name`'s number, if it's numbered.
    #[must_use]
    pub(crate) fn number<Q>(&self, name: &Q) -> Option<u32>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.numbers.get(name).copied()
    }

    /// The name numbered `number`, if one is.
    #[must_use]
    pub(crate) fn name(&self, number: u32) -> Option<&K> {
        self.names.get(number as usize)?.as_ref()
    }

    /// How many numbers there are, empty ones too: one past the last.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether nothing's numbered, or ever was.
    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Every name with its number, in order of number.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &K)> {
        (0u32..)
            .zip(&self.names)
            .filter_map(|(number, name)| Some((number, name.as_ref()?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_numbered_in_the_order_given() {
        let numbering = Numbering::default().renumbered(["a", "b", "c"]);
        assert_eq!(numbering.number("b"), Some(1));
        assert_eq!(numbering.name(2), Some(&"c"));
        assert_eq!(numbering.number("d"), None);
    }

    #[test]
    fn renumbered_a_name_keeps_its_number() {
        let first = Numbering::default().renumbered(["a", "b", "c"]);
        let next = first.renumbered(["a", "aa", "c"]);
        assert_eq!(next.number("a"), Some(0));
        assert_eq!(next.number("c"), Some(2));
        assert_eq!(next.number("aa"), Some(3), "new after the last");
        assert_eq!(next.name(1), None, "gone, its number empty");
        assert_eq!(next.len(), 4);
        let numbered: Vec<_> = next.iter().collect();
        assert_eq!(numbered, [(0, &"a"), (2, &"c"), (3, &"aa")]);
    }
}
