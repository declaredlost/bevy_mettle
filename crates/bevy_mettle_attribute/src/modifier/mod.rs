//! Modifiers: what sources hold, and attributes fold.

mod operation;
mod sources;

use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::tag::{Tag, TagTree};
use bevy_mettle_core::{Id, MettleError};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub use self::operation::Operation;
pub use self::sources::{ModifierOf, ModifierSources, Modifiers};
use crate::{AnyAttribute, Attribute, AttributeRegistry};

/// A modifier: one line on an item, a passive, an effect. Changes an
/// attribute by an [op](Operation) and a value, while its [`Condition`]
/// holds and as much as it does, once for each whole so much of another
/// attribute if it's [`per`](Self::per) one.
///
/// ```ron
/// (attribute: "damage", op: Additive, value: 0.2, condition: ReadTagged("damage.elemental.fire"))
/// (attribute: "resistance", op: Flat, value: 0.1, condition: ReadTagged("damage.elemental"))
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Modifier {
    /// The attribute it changes.
    pub attribute: Id<AnyAttribute>,
    /// How.
    pub op: Operation,
    /// By how much: an amount for `Base`, `Flat`, `Unscaled` and
    /// `Override`, a fraction (`0.2` for +20%) for the rest. Negative for
    /// less.
    pub value: f32,
    /// When it counts, and how much. Always, if `None`.
    pub condition: Option<Condition>,
    /// That it counts once for each whole `every` of an attribute, read in
    /// the same context.
    pub per: Option<Per>,
    /// That it only counts for reads naming its source: a weapon's own
    /// damage, not every attack's.
    pub local: bool,
}

/// A modifier as data is written: what isn't there left out, what is
/// written bare (`condition: ReadTagged("fire")`).
#[derive(Serialize, Deserialize)]
#[serde(rename = "Modifier", deny_unknown_fields)]
struct Readable {
    attribute: Id<AnyAttribute>,
    op: Operation,
    value: f32,
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    condition: Option<Condition>,
    #[serde(
        default,
        with = "bevy_mettle_core::optional",
        skip_serializing_if = "Option::is_none"
    )]
    per: Option<Per>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    local: bool,
}

/// A modifier as a binary format writes it: every field, in order.
#[derive(Serialize, Deserialize)]
#[serde(rename = "Modifier")]
struct Packed {
    attribute: Id<AnyAttribute>,
    op: Operation,
    value: f32,
    condition: Option<Condition>,
    per: Option<Per>,
    local: bool,
}

macro_rules! convert {
    ($($shape:ident),*) => {$(
        impl From<Modifier> for $shape {
            fn from(modifier: Modifier) -> Self {
                Self {
                    attribute: modifier.attribute,
                    op: modifier.op,
                    value: modifier.value,
                    condition: modifier.condition,
                    per: modifier.per,
                    local: modifier.local,
                }
            }
        }

        impl From<$shape> for Modifier {
            fn from(shape: $shape) -> Self {
                Self {
                    attribute: shape.attribute,
                    op: shape.op,
                    value: shape.value,
                    condition: shape.condition,
                    per: shape.per,
                    local: shape.local,
                }
            }
        }
    )*};
}

convert!(Readable, Packed);

/// As data (RON, JSON), leaving out what isn't there; in a binary format
/// (what networking uses), every field.
impl Serialize for Modifier {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            Readable::from(self.clone()).serialize(serializer)
        } else {
            Packed::from(self.clone()).serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Modifier {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            Readable::deserialize(deserializer).map(Self::from)
        } else {
            Packed::deserialize(deserializer).map(Self::from)
        }
    }
}

/// That a modifier counts once for each whole `every` of `attribute`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Per {
    /// The attribute it scales with.
    pub attribute: Id<AnyAttribute>,
    /// How much of it makes one: more than 0.
    pub every: f32,
}

impl Modifier {
    /// `op` of `value` to `attribute`, everywhere and always.
    #[must_use]
    pub fn new(attribute: Id<AnyAttribute>, op: Operation, value: f32) -> Self {
        Self {
            attribute,
            op,
            value,
            condition: None,
            per: None,
            local: false,
        }
    }

    /// Replaces `A`'s base.
    #[must_use]
    pub fn base<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Base, value)
    }

    /// Adds `value` to `A`'s base.
    #[must_use]
    pub fn flat<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Flat, value)
    }

    /// `A` by a fraction (`0.2` for +20%), summed with the other additive
    /// ones.
    #[must_use]
    pub fn additive<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Additive, value)
    }

    /// `A` by a fraction (`0.2` for +20%), multiplied in on its own.
    #[must_use]
    pub fn multiplicative<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Multiplicative, value)
    }

    /// `A` divided by one more than a fraction, summed with the other
    /// dividing ones.
    #[must_use]
    pub fn divide<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Divide, value)
    }

    /// Adds `value` to `A` after everything else.
    #[must_use]
    pub fn unscaled<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Unscaled, value)
    }

    /// `A` is `value`, whatever else there is.
    #[must_use]
    pub fn overriding<A: Attribute>(value: f32) -> Self {
        Self::new(A::ID, Operation::Override, value)
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
    pub fn read_tagged(self, tag: impl Into<Id<Tag>>) -> Self {
        self.condition(Condition::read_tagged(tag))
    }

    /// Only counting while the owner owns `tag`, or one under it.
    #[must_use]
    pub fn owner_has(self, tag: impl Into<Id<Tag>>) -> Self {
        self.condition(Condition::owner_has(tag))
    }

    /// Counting once for each whole `every` of `A` (more than 0).
    #[must_use]
    pub fn per<A: Attribute>(mut self, every: f32) -> Self {
        self.per = Some(Per {
            attribute: A::ID,
            every,
        });
        self
    }

    /// Only counting for reads naming its source.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

    /// Whether it always counts: no condition, not local, not per
    /// anything. What folding sums at once.
    pub(crate) fn always(&self) -> bool {
        self.condition.is_none() && self.per.is_none() && !self.local
    }

    /// Checks it: its attributes are in `attributes`, its tags in `tags`,
    /// its numbers ones that can be.
    ///
    /// # Errors
    ///
    /// The first that isn't.
    pub fn check(&self, attributes: &AttributeRegistry, tags: &TagTree) -> Result<(), MettleError> {
        self.check_values(attributes)?;
        match &self.condition {
            Some(condition) => condition.check(tags, None),
            None => Ok(()),
        }
    }

    /// Checks its attributes and numbers, not its tags: what folding
    /// needs, without the tag tree.
    pub(crate) fn check_values(&self, attributes: &AttributeRegistry) -> Result<(), MettleError> {
        let invalid = |what: String| MettleError::Invalid { what, within: None };
        attributes.check(&self.attribute, None)?;
        if !self.value.is_finite() {
            return Err(invalid(format!(
                "a modifier of `{}` by {}",
                self.attribute, self.value
            )));
        }
        if let Some(per) = &self.per {
            attributes.check(&per.attribute, None)?;
            if !(per.every.is_finite() && per.every > 0.0) {
                return Err(invalid(format!(
                    "a modifier of `{}` per {} {}",
                    self.attribute, per.every, per.attribute
                )));
            }
        }
        Ok(())
    }
}
