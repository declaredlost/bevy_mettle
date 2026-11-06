//! How much: a number in an effect's definition.

use bevy_mettle_attribute::registry::AnyAttribute;
use bevy_mettle_core::Id;
use bevy_mettle_core::curve::LevelCurve;
use bevy_mettle_core::tag::Tag;
use serde::{Deserialize, Serialize};

use crate::code::Calculation;

/// How much: a modifier's value, a duration or a period, from the spec.
///
/// Worked out as the effect's applied, from its spec.
///
/// ```ron
/// Value(4)
/// Scaled(base: 1.0, curve: "fireball.damage")
/// Param("param.damage")
/// Attribute(attribute: "strength", from: Source, coefficient: 0.5)
/// Custom("armour_break")
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
        curve: Id<LevelCurve>,
    },
    /// What the code applying it set, by a tag (`param.damage`: Unreal's
    /// `SetByCaller`); 0, with a warning, if it didn't.
    Param(Id<Tag>),
    /// An attribute of the source's or the target's, as it is when the effect's
    /// applied, times `coefficient`.
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
    Custom(Id<Calculation>),
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
            attribute: A::ID,
            from: Capture::Source,
            coefficient,
        }
    }

    /// `A` of the target's, as it is when the effect's applied.
    #[must_use]
    pub fn target<A: bevy_mettle_attribute::Attribute>(coefficient: f32) -> Self {
        Self::Attribute {
            attribute: A::ID,
            from: Capture::Target,
            coefficient,
        }
    }

    /// What the code applying it set as `param`.
    #[must_use]
    pub fn param(param: impl Into<Id<Tag>>) -> Self {
        Self::Param(param.into())
    }

    /// `base` times `curve` at the spec's level.
    #[must_use]
    pub fn scaled(base: f32, curve: impl Into<Id<LevelCurve>>) -> Self {
        Self::Scaled {
            base,
            curve: curve.into(),
        }
    }

    /// The attribute an `Attribute` magnitude reads, if it's one.
    pub(crate) fn attribute(&self) -> Option<&Id<AnyAttribute>> {
        match self {
            Self::Attribute { attribute, .. } => Some(attribute),
            _ => None,
        }
    }
}

impl From<f32> for Magnitude {
    fn from(value: f32) -> Self {
        Self::Value(value)
    }
}
