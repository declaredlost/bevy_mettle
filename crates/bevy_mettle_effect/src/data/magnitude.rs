//! How much: a number in an effect's definition.

use bevy_mettle_attribute::registry::AnyAttribute;
use bevy_mettle_core::Id;
use bevy_mettle_core::curve::AnyCurve;
use bevy_mettle_core::tag::TagKey;
use serde::{Deserialize, Serialize};

use crate::code::AnyCalculation;

/// How much: a modifier's value, a duration or a period, from the spec.
///
/// Worked out as the effect's applied, from its spec.
///
/// ```
/// # use bevy_mettle_effect::prelude::*;
/// let four = Magnitude::from(4.0);
/// let by_level = Magnitude::scaled(1.0, "fireball.damage");
/// let set_by_code = Magnitude::param("param.damage");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Magnitude {
    /// This value.
    Value(f32),
    /// `base` times `curve` at the spec's level (Unreal's `ScalableFloat`).
    Scaled {
        /// What the curve's value is multiplied by.
        base: f32,
        /// The curve.
        curve: Id<AnyCurve>,
    },
    /// What the code applying it set, by a tag (`param.damage`: Unreal's
    /// `SetByCaller`); 0, with a warning, if it didn't.
    Param(TagKey),
    /// An attribute of the source's or the target's, as it is when the effect's
    /// applied, times `coefficient`.
    ///
    /// Captured then, and kept as it lasts: new stacks and periods use the
    /// value captured, and a client shows the server's. A new definition
    /// captures it again.
    Attribute {
        /// The attribute.
        attribute: Id<AnyAttribute>,
        /// Whose: the source's, unless it says.
        #[serde(default)]
        from: Capture,
        /// What it's multiplied by: 1 unless it says.
        #[serde(default = "one")]
        coefficient: f32,
    },
    /// Worked out by a calculation registered in code.
    Custom(Id<AnyCalculation>),
}

/// Whose attribute an `Attribute` magnitude reads.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize
)]
pub enum Capture {
    /// The source's: the caster, the attacker. None without a source.
    #[default]
    Source,
    /// The target's.
    Target,
}

fn one() -> f32 {
    1.0
}

impl Magnitude {
    /// `A` of the source's, as it is when the effect's applied.
    #[must_use]
    pub fn source<A: bevy_mettle_attribute::Attribute>(coefficient: f32) -> Self {
        Self::Attribute {
            attribute: Id::from_static(A::NAME),
            from: Capture::Source,
            coefficient,
        }
    }

    /// `A` of the target's, as it is when the effect's applied.
    #[must_use]
    pub fn target<A: bevy_mettle_attribute::Attribute>(coefficient: f32) -> Self {
        Self::Attribute {
            attribute: Id::from_static(A::NAME),
            from: Capture::Target,
            coefficient,
        }
    }

    /// What the code applying it set as `param`.
    #[must_use]
    pub fn param(param: impl Into<TagKey>) -> Self {
        Self::Param(param.into())
    }

    /// `base` times `curve` at the spec's level.
    #[must_use]
    pub fn scaled(base: f32, curve: impl Into<Id<AnyCurve>>) -> Self {
        Self::Scaled {
            base,
            curve: curve.into(),
        }
    }
}

impl From<f32> for Magnitude {
    fn from(value: f32) -> Self {
        Self::Value(value)
    }
}
