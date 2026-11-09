//! The condition language: expressions over tags and facts, weighed 0 to 1.

mod check;
mod context;
#[cfg(test)]
mod tests;

use std::ops;

use bevy_reflect::prelude::*;
use serde::{Deserialize, Serialize};

pub use self::context::Context;
use crate::tag::TagKey;

/// When something counts, and how much: from 0 (not at all) to 1 (in full).
///
/// Most terms hold or don't; [`FactRamp`](Self::FactRamp) holds in part, and so
/// do the combinators built on it. What a modifier counts while, an effect
/// needs to apply, an ability needs to activate.
///
/// ```
/// # use bevy_mettle_core::prelude::*;
/// let falloff = Condition::all([
///     Condition::read_tagged("delivery.attack"),
///     Condition::fact_ramp("fact.flown", 2.0, 8.0),
///     !Condition::target_has("creature.undead"),
/// ]);
/// ```
///
/// A tag term holds for the tag or any under it. A fact is a number a read
/// supplies, named by a tag (`fact.flown`); one the read didn't supply isn't
/// there: [`FactSet`](Self::FactSet) and the comparisons don't hold, so a read
/// with no hit gets no hit bonuses. Reflected as a value.
///
/// A condition over [`TagKey`]s, as this process knows its tags: what every
/// condition is but on the wire, where its tags are their numbers in the
/// [`TagRegistry`](crate::tag::TagRegistry), a `ConditionOver<TagId>`
/// ([`map_tags`](ConditionOver::map_tags)).
pub type Condition = ConditionOver<TagKey>;

/// A [`Condition`] over tags of `T`: [`TagKey`]s, or on the wire
/// [`TagId`](crate::tag::TagId)s.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
#[reflect(opaque, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[reflect(where T: Clone + PartialEq + std::fmt::Debug + Serialize + for<'de> Deserialize<'de>)]
pub enum ConditionOver<T> {
    /// Every one holds: the product of their weights.
    All(Vec<ConditionOver<T>>),
    /// Any one holds: the greatest of their weights.
    Any(Vec<ConditionOver<T>>),
    /// It doesn't hold: one less its weight.
    Not(Box<ConditionOver<T>>),
    /// The read has the tag: the hit's, the ability's, `delivery.attack`.
    ReadTagged(T),
    /// The owner owns the tag: `state.low_life`, `state.moving`.
    OwnerHas(T),
    /// The target owns the tag: `creature.undead`.
    TargetHas(T),
    /// The source owns the tag: the caster, the attacker, `state.enraged`.
    SourceHas(T),
    /// The fact is set, above 0: `fact.behind`.
    FactSet(T),
    /// The fact is above `value`: risen more than 2 tiles.
    FactAbove {
        /// The fact.
        fact: T,
        /// What it has to be above.
        value: f32,
    },
    /// The fact is below `value`: the target under a third of its life.
    FactBelow {
        /// The fact.
        fact: T,
        /// What it has to be below.
        value: f32,
    },
    /// In full where the fact is `full`, not at all where it's `none`, in
    /// proportion between, and as at the nearer end beyond: a falloff by how
    /// far a projectile flew. `full` below `none` fades as the fact grows
    /// (point blank), above it builds up (far shot).
    FactRamp {
        /// The fact.
        fact: T,
        /// Where it holds in full.
        full: f32,
        /// Where it doesn't hold at all.
        none: f32,
    },
    /// The fact, a count, is a whole multiple of `n`: every sixth hit. Holds
    /// for counts of 1 and up.
    FactEvery {
        /// The fact: a count.
        fact: T,
        /// Every how many: more than 0.
        n: u32,
    },
}

impl<T> ops::Not for ConditionOver<T> {
    type Output = Self;

    /// It doesn't hold: `!Condition::target_has(undead)`.
    fn not(self) -> Self {
        Self::Not(Box::new(self))
    }
}

impl<T: Copy> ConditionOver<T> {
    /// The same condition, each of its tags mapped by `map`: `None` if one
    /// doesn't map. What sends it by number, and reads it back.
    pub fn map_tags<U>(&self, mut map: impl FnMut(T) -> Option<U>) -> Option<ConditionOver<U>> {
        self.map_with(&mut map)
    }

    fn map_with<U>(&self, map: &mut dyn FnMut(T) -> Option<U>) -> Option<ConditionOver<U>> {
        let mut all = |terms: &[Self]| -> Option<Vec<ConditionOver<U>>> {
            terms.iter().map(|term| term.map_with(&mut *map)).collect()
        };
        Some(match self {
            Self::All(terms) => ConditionOver::All(all(terms)?),
            Self::Any(terms) => ConditionOver::Any(all(terms)?),
            Self::Not(term) => ConditionOver::Not(Box::new(term.map_with(map)?)),
            Self::ReadTagged(tag) => ConditionOver::ReadTagged(map(*tag)?),
            Self::OwnerHas(tag) => ConditionOver::OwnerHas(map(*tag)?),
            Self::TargetHas(tag) => ConditionOver::TargetHas(map(*tag)?),
            Self::SourceHas(tag) => ConditionOver::SourceHas(map(*tag)?),
            Self::FactSet(fact) => ConditionOver::FactSet(map(*fact)?),
            Self::FactAbove { fact, value } => ConditionOver::FactAbove {
                fact: map(*fact)?,
                value: *value,
            },
            Self::FactBelow { fact, value } => ConditionOver::FactBelow {
                fact: map(*fact)?,
                value: *value,
            },
            Self::FactRamp { fact, full, none } => ConditionOver::FactRamp {
                fact: map(*fact)?,
                full: *full,
                none: *none,
            },
            Self::FactEvery { fact, n } => ConditionOver::FactEvery {
                fact: map(*fact)?,
                n: *n,
            },
        })
    }
}

impl Condition {
    /// Every one of `terms` holds.
    #[must_use]
    pub fn all(terms: impl IntoIterator<Item = Condition>) -> Self {
        Self::All(terms.into_iter().collect())
    }

    /// Any one of `terms` holds.
    #[must_use]
    pub fn any(terms: impl IntoIterator<Item = Condition>) -> Self {
        Self::Any(terms.into_iter().collect())
    }

    /// The read has `tag`, or one under it.
    #[must_use]
    pub fn read_tagged(tag: impl Into<TagKey>) -> Self {
        Self::ReadTagged(tag.into())
    }

    /// The owner owns `tag`, or one under it.
    #[must_use]
    pub fn owner_has(tag: impl Into<TagKey>) -> Self {
        Self::OwnerHas(tag.into())
    }

    /// The target owns `tag`, or one under it.
    #[must_use]
    pub fn target_has(tag: impl Into<TagKey>) -> Self {
        Self::TargetHas(tag.into())
    }

    /// The source owns `tag`, or one under it.
    #[must_use]
    pub fn source_has(tag: impl Into<TagKey>) -> Self {
        Self::SourceHas(tag.into())
    }

    /// `fact` is above 0.
    #[must_use]
    pub fn fact_set(fact: impl Into<TagKey>) -> Self {
        Self::FactSet(fact.into())
    }

    /// `fact` is above `value`.
    #[must_use]
    pub fn fact_above(fact: impl Into<TagKey>, value: f32) -> Self {
        Self::FactAbove {
            fact: fact.into(),
            value,
        }
    }

    /// `fact` is below `value`.
    #[must_use]
    pub fn fact_below(fact: impl Into<TagKey>, value: f32) -> Self {
        Self::FactBelow {
            fact: fact.into(),
            value,
        }
    }

    /// In full where `fact` is `full`, not at all where it's `none`, in
    /// proportion between.
    #[must_use]
    pub fn fact_ramp(fact: impl Into<TagKey>, full: f32, none: f32) -> Self {
        Self::FactRamp {
            fact: fact.into(),
            full,
            none,
        }
    }

    /// `fact`, a count, is a whole multiple of `n`.
    #[must_use]
    pub fn fact_every(fact: impl Into<TagKey>, n: u32) -> Self {
        Self::FactEvery {
            fact: fact.into(),
            n,
        }
    }

    /// This and `term`: `term` added to an `All`, or the two made one.
    #[must_use]
    pub fn and(self, term: Condition) -> Self {
        match self {
            Self::All(mut terms) => {
                terms.push(term);
                Self::All(terms)
            }
            first => Self::All(vec![first, term]),
        }
    }

    /// How much it holds in `context`: from 0 to 1.
    pub fn weight(&self, context: &(impl Context + ?Sized)) -> f32 {
        let all = |holds: bool| if holds { 1.0 } else { 0.0 };
        match self {
            Self::All(terms) => terms.iter().map(|term| term.weight(context)).product(),
            Self::Any(terms) => terms
                .iter()
                .map(|term| term.weight(context))
                .fold(0.0, f32::max),
            Self::Not(term) => 1.0 - term.weight(context),
            Self::ReadTagged(tag) => all(context.tag(*tag)),
            Self::OwnerHas(tag) => all(context.has(*tag)),
            Self::TargetHas(tag) => all(context.target(*tag)),
            Self::SourceHas(tag) => all(context.source(*tag)),
            Self::FactSet(fact) => all(context.fact(*fact).is_some_and(|value| value > 0.0)),
            Self::FactAbove { fact, value } => {
                all(context.fact(*fact).is_some_and(|it| it > *value))
            }
            Self::FactBelow { fact, value } => {
                all(context.fact(*fact).is_some_and(|it| it < *value))
            }
            Self::FactRamp { fact, full, none } => context.fact(*fact).map_or(0.0, |it| {
                (1.0 - (it - full) / (none - full)).clamp(0.0, 1.0)
            }),
            Self::FactEvery { fact, n } => all(context.fact(*fact).is_some_and(|it| {
                let count = f64::from(it).round();
                count >= 1.0 && count.rem_euclid(f64::from(*n)) < 0.5
            })),
        }
    }

    /// It with every `SourceHas` term settled by `owns`: what the source owned
    /// when an effect was applied, kept as it was. A term owned is always
    /// (`All([])`), one not never (`Any([])`).
    #[must_use]
    pub fn settle_source(&self, owns: &impl Fn(TagKey) -> bool) -> Self {
        match self {
            Self::All(terms) => {
                Self::All(terms.iter().map(|term| term.settle_source(owns)).collect())
            }
            Self::Any(terms) => {
                Self::Any(terms.iter().map(|term| term.settle_source(owns)).collect())
            }
            Self::Not(term) => Self::Not(Box::new(term.settle_source(owns))),
            Self::SourceHas(tag) if owns(*tag) => Self::All(Vec::new()),
            Self::SourceHas(_) => Self::Any(Vec::new()),
            other => other.clone(),
        }
    }

    /// Whether it holds in full in `context`: what an effect's or an ability's
    /// requirement asks.
    pub fn holds(&self, context: &(impl Context + ?Sized)) -> bool {
        self.weight(context) >= 1.0
    }
}
