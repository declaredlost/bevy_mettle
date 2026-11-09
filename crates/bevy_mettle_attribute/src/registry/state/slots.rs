//! What an actor keeps of each attribute, by the attribute's number.

use bevy::prelude::*;

use crate::registry::AttributeId;

/// Values by attribute number, kept densely: a read is an index, not a hash.
#[derive(Debug, Clone, Reflect)]
pub(crate) struct Slots<V> {
    values: Vec<Option<V>>,
}

/// The same values, however long either's kept: an attribute taken off past
/// the last leaves an empty slot, not a change.
impl<V: PartialEq> PartialEq for Slots<V> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<V> Default for Slots<V> {
    fn default() -> Self {
        Self { values: Vec::new() }
    }
}

impl<V> Slots<V> {
    /// `attribute`'s value, if it has one.
    pub(crate) fn get(&self, attribute: AttributeId) -> Option<&V> {
        self.values.get(attribute.index())?.as_ref()
    }

    /// Whether `attribute` has a value.
    pub(crate) fn contains(&self, attribute: AttributeId) -> bool {
        self.get(attribute).is_some()
    }

    /// Sets `attribute`'s value.
    pub(crate) fn insert(&mut self, attribute: AttributeId, value: V) {
        *self.slot(attribute) = Some(value);
    }

    /// `attribute`'s value, made the default if it has none.
    pub(crate) fn or_default(&mut self, attribute: AttributeId) -> &mut V
    where
        V: Default,
    {
        self.slot(attribute).get_or_insert_with(V::default)
    }

    /// Takes `attribute`'s value away.
    pub(crate) fn remove(&mut self, attribute: AttributeId) -> Option<V> {
        self.values.get_mut(attribute.index())?.take()
    }

    /// Takes every value away.
    pub(crate) fn clear(&mut self) {
        self.values.clear();
    }

    /// Every attribute with a value, and the value, by number.
    pub(crate) fn iter(&self) -> impl Iterator<Item = (AttributeId, &V)> {
        (0u32..).zip(&self.values).filter_map(|(index, value)| {
            value.as_ref().map(|value| (AttributeId::new(index), value))
        })
    }

    /// Every attribute with a value.
    pub(crate) fn keys(&self) -> impl Iterator<Item = AttributeId> + '_ {
        self.iter().map(|(attribute, _)| attribute)
    }

    fn slot(&mut self, attribute: AttributeId) -> &mut Option<V> {
        let index = attribute.index();
        if index >= self.values.len() {
            self.values.resize_with(index + 1, || None);
        }
        &mut self.values[index]
    }
}

impl<V> FromIterator<(AttributeId, V)> for Slots<V> {
    fn from_iter<I: IntoIterator<Item = (AttributeId, V)>>(iter: I) -> Self {
        let mut slots = Self::default();
        for (attribute, value) in iter {
            slots.insert(attribute, value);
        }
        slots
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_kept_longer_are_still_the_same() {
        let mut short = Slots::default();
        short.insert(AttributeId::new(0), 1.0_f32);
        let mut long = short.clone();
        long.insert(AttributeId::new(3), 2.0);
        long.remove(AttributeId::new(3));
        assert_eq!(short, long);
        long.insert(AttributeId::new(3), 2.0);
        assert_ne!(short, long);
    }
}
