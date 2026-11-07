//! How a modifier changes its attribute.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// How a modifier changes its attribute. An attribute is
///
/// ```text
/// ((base + Σ flat) × (1 + Σ additive) × Π (1 + multiplicative) / (1 + Σ divide)) + Σ unscaled
/// ```
///
/// unless overridden, then rounded and kept within its limits. Of `Base`s and
/// `Override`s, the one applied last counts, as Unreal's does: a source's as
/// its `ModifierOf` was last set (an effect applied, an item equipped), the
/// actor's own as its `Modifiers` was added; within one, the later in its
/// list; sources applied in one tick, by entity. On the server that's exact,
/// and it decides: a client shows the values the server serves
/// ([`Served`](crate::authority::Served)), not its own fold, but while it
/// predicts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, Reflect)]
#[reflect(Clone, Debug, Hash, PartialEq, Serialize, Deserialize)]
pub enum Operation {
    /// Replaces the base: the one applied last counts.
    Base,
    /// Added to the base.
    Flat,
    /// A fraction (`0.2` for +20%), summed with the others of its kind, then
    /// multiplied in: +20% and +30% are +50%. "Increased".
    Additive,
    /// A fraction (`0.2` for +20%), multiplied in on its own: +20% and +30% are
    /// +56%. "More".
    Multiplicative,
    /// A fraction, summed with the others of its kind, then divided by: +100%
    /// halves it, and more never reaches 0. Cooldown recovery. Less than
    /// -99% divides by 0.01, a hundred times at most.
    Divide,
    /// Added after everything else: "+10 damage, not affected by increases".
    Unscaled,
    /// The value, whatever else there is: the one applied last counts.
    Override,
}

impl Operation {
    /// Whether it only counts in full: a base or an override is there or not;
    /// every other is an amount, which counts in part.
    #[must_use]
    pub fn whole(self) -> bool {
        matches!(self, Self::Base | Self::Override)
    }

    /// `base` changed by `value` as an instant effect changes it: `Flat` and
    /// `Unscaled` add, `Additive` and `Multiplicative` scale, `Divide`
    /// divides, `Base` and `Override` set.
    #[must_use]
    pub fn apply(self, base: f32, value: f32) -> f32 {
        match self {
            Self::Base | Self::Override => value,
            Self::Flat | Self::Unscaled => base + value,
            Self::Additive | Self::Multiplicative => base * (1.0 + value),
            Self::Divide => base / crate::registry::state::divisor(value),
        }
    }
}
