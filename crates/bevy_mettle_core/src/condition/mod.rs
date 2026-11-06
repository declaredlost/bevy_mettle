//! The condition language: expressions over tags and facts, weighed 0 to 1.

mod check;
mod context;
#[cfg(test)]
mod tests;

use std::ops;

use serde::{Deserialize, Serialize};

pub use self::context::Context;
use crate::Id;
use crate::tag::Tag;

/// When something counts, and how much: from 0 (not at all) to 1 (in full).
///
/// Most terms hold or don't; [`FactRamp`](Self::FactRamp) holds in part, and so
/// do the combinators built on it. What a modifier counts while, an effect
/// needs to apply, an ability needs to activate.
///
/// ```ron
/// All([
///     ReadTagged("delivery.attack"),
///     FactRamp(fact: "fact.flown", full: 2, none: 8),
///     Not(TargetHas("creature.undead")),
/// ])
/// ```
///
/// A tag term holds for the tag or any under it. A fact is a number a read
/// supplies, named by a tag (`fact.flown`); one the read didn't supply isn't
/// there: [`FactSet`](Self::FactSet) and the comparisons don't hold, so a read
/// with no hit gets no hit bonuses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Condition {
    /// Every one holds: the product of their weights.
    All(Vec<Condition>),
    /// Any one holds: the greatest of their weights.
    Any(Vec<Condition>),
    /// It doesn't hold: one less its weight.
    Not(Box<Condition>),
    /// The read has the tag: the hit's, the ability's, `delivery.attack`.
    ReadTagged(Id<Tag>),
    /// The owner owns the tag: `state.low_life`, `state.moving`.
    OwnerHas(Id<Tag>),
    /// The target owns the tag: `creature.undead`.
    TargetHas(Id<Tag>),
    /// The source owns the tag: the caster, the attacker, `state.enraged`.
    SourceHas(Id<Tag>),
    /// The fact is set, above 0: `fact.behind`.
    FactSet(Id<Tag>),
    /// The fact is above `value`: risen more than 2 tiles.
    FactAbove {
        /// The fact.
        fact: Id<Tag>,
        /// What it has to be above.
        value: f32,
    },
    /// The fact is below `value`: the target under a third of its life.
    FactBelow {
        /// The fact.
        fact: Id<Tag>,
        /// What it has to be below.
        value: f32,
    },
    /// In full where the fact is `full`, not at all where it's `none`, in
    /// proportion between, and as at the nearer end beyond: a falloff by how
    /// far a projectile flew. `full` below `none` fades as the fact grows
    /// (point blank), above it builds up (far shot).
    FactRamp {
        /// The fact.
        fact: Id<Tag>,
        /// Where it holds in full.
        full: f32,
        /// Where it doesn't hold at all.
        none: f32,
    },
    /// The fact, a count, is a whole multiple of `n`: every sixth hit. Holds
    /// for counts of 1 and up.
    FactEvery {
        /// The fact: a count.
        fact: Id<Tag>,
        /// Every how many: more than 0.
        n: u32,
    },
}

impl ops::Not for Condition {
    type Output = Self;

    /// It doesn't hold: `!Condition::target_has(undead)`.
    fn not(self) -> Self {
        Self::Not(Box::new(self))
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
    pub fn read_tagged(tag: impl Into<Id<Tag>>) -> Self {
        Self::ReadTagged(tag.into())
    }

    /// The owner owns `tag`, or one under it.
    #[must_use]
    pub fn owner_has(tag: impl Into<Id<Tag>>) -> Self {
        Self::OwnerHas(tag.into())
    }

    /// The target owns `tag`, or one under it.
    #[must_use]
    pub fn target_has(tag: impl Into<Id<Tag>>) -> Self {
        Self::TargetHas(tag.into())
    }

    /// The source owns `tag`, or one under it.
    #[must_use]
    pub fn source_has(tag: impl Into<Id<Tag>>) -> Self {
        Self::SourceHas(tag.into())
    }

    /// `fact` is above 0.
    #[must_use]
    pub fn fact_set(fact: impl Into<Id<Tag>>) -> Self {
        Self::FactSet(fact.into())
    }

    /// `fact` is above `value`.
    #[must_use]
    pub fn fact_above(fact: impl Into<Id<Tag>>, value: f32) -> Self {
        Self::FactAbove {
            fact: fact.into(),
            value,
        }
    }

    /// `fact` is below `value`.
    #[must_use]
    pub fn fact_below(fact: impl Into<Id<Tag>>, value: f32) -> Self {
        Self::FactBelow {
            fact: fact.into(),
            value,
        }
    }

    /// In full where `fact` is `full`, not at all where it's `none`, in
    /// proportion between.
    #[must_use]
    pub fn fact_ramp(fact: impl Into<Id<Tag>>, full: f32, none: f32) -> Self {
        Self::FactRamp {
            fact: fact.into(),
            full,
            none,
        }
    }

    /// `fact`, a count, is a whole multiple of `n`.
    #[must_use]
    pub fn fact_every(fact: impl Into<Id<Tag>>, n: u32) -> Self {
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
            Self::ReadTagged(tag) => all(context.tag(tag)),
            Self::OwnerHas(tag) => all(context.has(tag)),
            Self::TargetHas(tag) => all(context.target(tag)),
            Self::SourceHas(tag) => all(context.source(tag)),
            Self::FactSet(fact) => all(context.fact(fact).is_some_and(|value| value > 0.0)),
            Self::FactAbove { fact, value } => {
                all(context.fact(fact).is_some_and(|it| it > *value))
            }
            Self::FactBelow { fact, value } => {
                all(context.fact(fact).is_some_and(|it| it < *value))
            }
            Self::FactRamp { fact, full, none } => context.fact(fact).map_or(0.0, |it| {
                (1.0 - (it - full) / (none - full)).clamp(0.0, 1.0)
            }),
            Self::FactEvery { fact, n } => all(context.fact(fact).is_some_and(|it| {
                let count = f64::from(it).round();
                count >= 1.0 && count.rem_euclid(f64::from(*n)) < 0.5
            })),
        }
    }

    /// It with every `SourceHas` term settled by `owns`: what the source owned
    /// when an effect was applied, kept as it was. A term owned is always
    /// (`All([])`), one not never (`Any([])`).
    #[must_use]
    pub fn settle_source(&self, owns: &impl Fn(&Id<Tag>) -> bool) -> Self {
        match self {
            Self::All(terms) => {
                Self::All(terms.iter().map(|term| term.settle_source(owns)).collect())
            }
            Self::Any(terms) => {
                Self::Any(terms.iter().map(|term| term.settle_source(owns)).collect())
            }
            Self::Not(term) => Self::Not(Box::new(term.settle_source(owns))),
            Self::SourceHas(tag) if owns(tag) => Self::All(Vec::new()),
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
