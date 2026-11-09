//! Modifiers: what sources hold, and attributes fold into their current values.

mod operation;
mod replica;
mod sources;
mod spec;

use bevy::prelude::*;
use bevy_mettle_core::Error;
use bevy_mettle_core::condition::{Condition, ConditionOver};
use bevy_mettle_core::registry::AttributeId;
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use serde::{Deserialize, Serialize};

pub use self::operation::Operation;
pub use self::replica::ModifiersReplica;
pub(crate) use self::replica::{show_replicas, sync_replicas};
pub use self::sources::{ModifierOf, ModifierSources, Modifiers, Provisional};
pub use self::spec::{ModifierCommands, ModifierSpec};
use crate::registry::AttributeRegistry;

/// A modifier: one line on an item, a passive or an effect.
///
/// Changes an attribute by an [op](Operation) and a value, while its
/// [`Condition`] holds and as much as it does, once for each whole so much of
/// another attribute if it's [`per`](Self::per) one.
///
/// It names its attribute by number, as an app has it, as Bevy names a
/// component by its `ComponentId`. Code writes one by type, as a
/// [`ModifierSpec`], which [`modifiers`](ModifierCommands::modifiers) makes a
/// `Modifier` as it's put on an entity; with a number in hand (from
/// [`AttributeRegistry`]), make one directly. What's written in data, an item's
/// stats, is an effect's modifier, which names its attribute by name.
///
/// Its condition's tags are [`TagKey`]s, `T`; sent over a network, a
/// `Modifier<TagId>` ([`map_tags`](Self::map_tags), [`ModifiersReplica`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(opaque, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(where T: Clone + PartialEq + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>)]
pub struct Modifier<T = TagKey> {
    /// The attribute it changes.
    pub attribute: AttributeId,
    /// How.
    pub op: Operation,
    /// By how much: an amount for `Base`, `Flat`, `Unscaled` and `Override`, a
    /// fraction (`0.2` for +20%) for the rest. Negative for less.
    pub value: f32,
    /// When it counts, and how much. Always, if `None`.
    pub condition: Option<ConditionOver<T>>,
    /// That it counts once for each whole `every` of an attribute, read in the
    /// same context. Only an amount can: not a `Base` or an `Override`.
    pub per: Option<Per>,
    /// That it only counts for reads naming its source: a weapon's own damage,
    /// not every attack's.
    pub local: bool,
}

/// That a modifier counts once for each whole `every` of `attribute`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Per {
    /// The attribute it scales with.
    pub attribute: AttributeId,
    /// How much of it makes one: more than 0.
    pub every: f32,
}

impl Per {
    /// How many whole `every` there are in `of`: none for less than one.
    ///
    /// Floored a little above, so `0.3` of `0.1` is 3 though the floats say
    /// 2.9999998.
    #[must_use]
    pub fn times(&self, of: f32) -> f32 {
        crate::registry::state::times_per(of, self.every)
    }
}

impl<T: Copy> Modifier<T> {
    /// The same modifier, its condition's tags mapped by `map`: `None` if one
    /// doesn't map. What sends it by number, and reads it back.
    pub fn map_tags<U>(&self, map: impl FnMut(T) -> Option<U>) -> Option<Modifier<U>> {
        let condition = match &self.condition {
            Some(condition) => Some(condition.map_tags(map)?),
            None => None,
        };
        Some(Modifier {
            attribute: self.attribute,
            op: self.op,
            value: self.value,
            condition,
            per: self.per.clone(),
            local: self.local,
        })
    }
}

impl Modifier {
    /// `op` of `value` to `attribute`, everywhere and always.
    #[must_use]
    pub fn new(attribute: AttributeId, op: Operation, value: f32) -> Self {
        Self {
            attribute,
            op,
            value,
            condition: None,
            per: None,
            local: false,
        }
    }

    /// Replaces `attribute`'s base.
    #[must_use]
    pub fn base(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Base, value)
    }

    /// Adds `value` to `attribute`'s base.
    #[must_use]
    pub fn flat(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Flat, value)
    }

    /// `attribute` by a fraction (`0.2` for +20%), summed with the other
    /// additive ones.
    #[must_use]
    pub fn additive(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Additive, value)
    }

    /// `attribute` by a fraction (`0.2` for +20%), multiplied in on its own.
    #[must_use]
    pub fn multiplicative(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Multiplicative, value)
    }

    /// `attribute` divided by one more than a fraction, summed with the other
    /// dividing ones.
    #[must_use]
    pub fn divide(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Divide, value)
    }

    /// Adds `value` to `attribute` after everything else.
    #[must_use]
    pub fn unscaled(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Unscaled, value)
    }

    /// `attribute` is `value`, whatever else there is.
    #[must_use]
    pub fn overriding(attribute: AttributeId, value: f32) -> Self {
        Self::new(attribute, Operation::Override, value)
    }

    /// Only counting as `condition` says, and as much: added to any other
    /// condition it has, all of which have to hold.
    #[must_use]
    pub fn condition(mut self, condition: Condition) -> Self {
        self.condition = Some(match self.condition.take() {
            Some(before) => before.and(condition),
            None => condition,
        });
        self
    }

    /// Only counting where the read has `tag`, or one under it.
    #[must_use]
    pub fn read_tagged(self, tag: impl Into<TagKey>) -> Self {
        self.condition(Condition::read_tagged(tag))
    }

    /// Only counting while the owner owns `tag`, or one under it.
    #[must_use]
    pub fn owner_has(self, tag: impl Into<TagKey>) -> Self {
        self.condition(Condition::owner_has(tag))
    }

    /// Counting once for each whole `every` of `attribute` (more than 0).
    ///
    /// Not for a `Base` or an `Override`, which count only in full: such a
    /// modifier is skipped.
    #[must_use]
    pub fn per(mut self, attribute: AttributeId, every: f32) -> Self {
        self.per = Some(Per { attribute, every });
        self
    }

    /// Only counting for reads naming its source.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

    /// Whether it always counts: no condition, not local, not per anything.
    /// What folding sums at once.
    pub(crate) fn always(&self) -> bool {
        self.condition.is_none() && self.per.is_none() && !self.local
    }

    /// Checks it: its attributes are in `attributes`, its tags in `tags`, its
    /// numbers ones that can be.
    ///
    /// # Errors
    ///
    /// The first that isn't.
    pub fn check(&self, attributes: &AttributeRegistry, tags: &TagRegistry) -> Result<(), Error> {
        self.check_values(attributes)?;
        match &self.condition {
            Some(condition) => condition.check(tags, None),
            None => Ok(()),
        }
    }

    /// Checks its attributes and numbers, its condition's too, not its tags:
    /// what folding needs, without the tag tree.
    pub(crate) fn check_values(&self, attributes: &AttributeRegistry) -> Result<(), Error> {
        let invalid = |what: String| Error::Invalid { what, within: None };
        let name = |attribute: AttributeId| {
            attributes
                .get(attribute)
                .map(|info| info.name)
                .ok_or_else(|| {
                    invalid(format!(
                        "a modifier of attribute {}, which there isn't",
                        attribute.index()
                    ))
                })
        };
        let changed = name(self.attribute)?;
        if !self.value.is_finite() {
            return Err(invalid(format!(
                "a modifier of `{changed}` by {}",
                self.value
            )));
        }
        if let Some(condition) = &self.condition {
            condition.check_values(None)?;
        }
        if let Some(per) = &self.per {
            let of = name(per.attribute)?;
            if self.op.whole() {
                return Err(invalid(format!(
                    "a {:?} of `{changed}` per `{of}`: only an amount can count per something",
                    self.op
                )));
            }
            if !(per.every.is_finite() && per.every > 0.0) {
                return Err(invalid(format!(
                    "a modifier of `{changed}` per {} `{of}`",
                    per.every
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn per_counts_whole_steps_despite_float_error() {
        let per = Per {
            attribute: AttributeId::new(0),
            every: 0.1,
        };
        assert_eq!(per.times(0.3), 3.0);
        assert_eq!(per.times(0.29), 2.0);
        assert_eq!(per.times(-1.0), 0.0);
    }

    #[test]
    fn an_operation_changes_a_base_as_an_instant_effect_does() {
        assert_eq!(Operation::Flat.apply(10.0, 5.0), 15.0);
        assert_eq!(Operation::Additive.apply(10.0, 0.5), 15.0);
        assert_eq!(Operation::Override.apply(10.0, 3.0), 3.0);
        assert_eq!(Operation::Divide.apply(10.0, 1.0), 5.0);
        assert!((Operation::Divide.apply(1.0, -2.0) - 100.0).abs() < 1e-3);
    }
}
