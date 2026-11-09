//! An attribute's modifiers summed, and those kept apart to weigh on reads.

use bevy::prelude::*;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::AttributeId;
use bevy_mettle_core::tag::TagCounts;
use serde::{Deserialize, Serialize};

use crate::modifier::{Modifier, Operation};
use crate::read::{AttributeContext, Read};

/// When a modifier counts, and how much: its expression, and whether it's local
/// to its source.
#[derive(Clone)]
pub(super) struct Counts {
    pub(super) condition: Option<Condition>,
    pub(super) local: bool,
    pub(super) source: Entity,
}

impl Counts {
    pub(super) fn of(modifier: &Modifier, source: Entity) -> Self {
        Self {
            condition: modifier.condition.clone(),
            local: modifier.local,
            source,
        }
    }

    /// How much it counts for `context`, with the owner owning `owned`: from 0
    /// to 1. A local one counts only for reads naming its source.
    pub(super) fn weight(&self, context: &AttributeContext, owned: &TagCounts) -> f32 {
        if self.local && context.source != Some(self.source) {
            return 0.0;
        }
        let weight = self
            .condition
            .as_ref()
            .map_or(1.0, |condition| condition.weight(&Read { context, owned }));
        // A weight that isn't a number counts for nothing.
        if weight.is_nan() { 0.0 } else { weight }
    }
}

/// A modifier that only sometimes counts.
#[derive(Clone)]
pub(super) struct Scoped {
    pub(super) op: Operation,
    pub(super) value: f32,
    /// The attribute it's per, and how much of it makes one.
    pub(super) per: Option<(AttributeId, f32)>,
    pub(super) counts: Counts,
    /// Where it comes, its actor's modifiers in the order they were applied:
    /// of `Base`s and `Override`s, the last counts.
    pub(super) order: u32,
}

/// An attribute's modifiers, summed but not applied: for tooltips and hits.
///
/// For an attacker's and a target's to be combined.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Breakdown {
    /// The base: set, or replaced by a `Base` modifier.
    pub base: f32,
    /// Σ flat.
    pub flat: f32,
    /// Σ additive.
    pub additive: f32,
    /// Π (1 + multiplicative).
    pub multiplier: f32,
    /// Σ divide.
    pub divide: f32,
    /// Σ unscaled.
    pub unscaled: f32,
    /// The override, if any.
    pub overriding: Option<f32>,
}

impl Breakdown {
    /// `((base + Σ flat) × (1 + Σ additive) × Π (1 + multiplicative) / (1 + Σ
    /// divide)) + Σ unscaled`, or the override.
    ///
    /// Less than -100% additive leaves nothing, not less; a divide of -99% or
    /// less divides by 0.01, a hundred times at most.
    #[must_use]
    pub fn total(&self) -> f32 {
        if let Some(value) = self.overriding {
            return value;
        }
        (self.base + self.flat) * (1.0 + self.additive).max(0.0) * self.multiplier
            / divisor(self.divide)
            + self.unscaled
    }
}

/// The least a divide divides by: -99% or less, a hundred times at most.
pub(crate) const LEAST_DIVISOR: f32 = 0.01;

/// What a divide of `divide` (`1.0` for +100%) divides by.
pub(crate) fn divisor(divide: f32) -> f32 {
    (1.0 + divide).max(LEAST_DIVISOR)
}

/// An attribute's always-counting modifiers, summed. A `Base` or an `Override`
/// is kept with where it comes, so the last counts however they're summed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sum {
    base: Option<(u32, f32)>,
    flat: f32,
    additive: f32,
    multiplier: f32,
    divide: f32,
    unscaled: f32,
    overriding: Option<(u32, f32)>,
}

impl Default for Sum {
    fn default() -> Self {
        Self {
            base: None,
            flat: 0.0,
            additive: 0.0,
            multiplier: 1.0,
            divide: 0.0,
            unscaled: 0.0,
            overriding: None,
        }
    }
}

impl Sum {
    /// Adds `value` by `op`, counted `weight` of itself, `times` times: an
    /// amount scaled by both, a base or an override only in full and once.
    ///
    /// A multiplicative one is multiplied in once each time, as its own:
    /// +10% twice is +21%.
    pub(super) fn add_weighted(
        &mut self,
        op: Operation,
        value: f32,
        weight: f32,
        times: f32,
        order: u32,
    ) {
        if times <= 0.0 {
            return;
        }
        match op {
            Operation::Base | Operation::Override => {
                if weight >= 1.0 {
                    self.add(op, value, order);
                }
            }
            Operation::Multiplicative => {
                self.multiplier *= (1.0 + value * weight).max(0.0).powf(times);
            }
            _ => self.add(op, value * weight * times, order),
        }
    }

    /// Adds `value` by `op`, the modifier coming `order`th.
    pub(super) fn add(&mut self, op: Operation, value: f32, order: u32) {
        match op {
            Operation::Base => self.base = later(self.base, Some((order, value))),
            Operation::Flat => self.flat += value,
            Operation::Additive => self.additive += value,
            Operation::Multiplicative => self.multiplier *= (1.0 + value).max(0.0),
            Operation::Divide => self.divide += value,
            Operation::Unscaled => self.unscaled += value,
            Operation::Override => self.overriding = later(self.overriding, Some((order, value))),
        }
    }

    /// The sum over `base`, unless a `Base` modifier replaces it.
    pub(super) fn over(&self, base: f32) -> Breakdown {
        Breakdown {
            base: self.base.map_or(base, |(_, value)| value),
            flat: self.flat,
            additive: self.additive,
            multiplier: self.multiplier,
            divide: self.divide,
            unscaled: self.unscaled,
            overriding: self.overriding.map(|(_, value)| value),
        }
    }
}

/// Of two values kept with where they come, the later.
fn later(one: Option<(u32, f32)>, other: Option<(u32, f32)>) -> Option<(u32, f32)> {
    match (one, other) {
        (Some(one), Some(other)) => Some(if other.0 >= one.0 { other } else { one }),
        (one, other) => one.or(other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_multiplicative_counted_twice_multiplies_twice() {
        let mut sum = Sum::default();
        sum.add_weighted(Operation::Multiplicative, 0.1, 1.0, 2.0, 0);
        assert!((sum.over(10.0).total() - 12.1).abs() < 1e-4);
    }

    #[test]
    fn a_whole_op_counted_no_times_counts_not_at_all() {
        let mut sum = Sum::default();
        sum.add_weighted(Operation::Base, 5.0, 1.0, 0.0, 0);
        assert_eq!(sum.over(10.0).total(), 10.0);
    }

    #[test]
    fn a_divide_of_minus_all_divides_by_a_hundredth() {
        let mut sum = Sum::default();
        sum.add(Operation::Divide, -3.0, 0);
        assert!((sum.over(1.0).total() - 100.0).abs() < 1e-3);
    }
}
