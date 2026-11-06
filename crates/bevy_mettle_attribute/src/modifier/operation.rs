//! How a modifier changes its attribute.

use serde::{Deserialize, Serialize};

/// How a modifier changes its attribute. An attribute is
///
/// ```text
/// ((base + Σ flat) × (1 + Σ additive) × Π (1 + multiplicative) / (1 + Σ divide)) + Σ unscaled
/// ```
///
/// unless overridden, then rounded and kept within its limits. Of `Base`s and
/// `Override`s, the last counts: in the actor's own modifiers, then each
/// source's, in the order the sources came.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize)]
pub enum Operation {
    /// Replaces the base: the last one counts.
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
    /// halves it, and more never reaches 0. Cooldown recovery.
    Divide,
    /// Added after everything else: "+10 damage, not affected by increases".
    Unscaled,
    /// The value, whatever else there is: the last one counts.
    Override,
}

impl Operation {
    /// Whether it only counts in full: a base or an override is there or not;
    /// every other is an amount, which counts in part.
    #[must_use]
    pub fn whole(self) -> bool {
        matches!(self, Self::Base | Self::Override)
    }
}
