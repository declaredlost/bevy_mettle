//! How much: a number in an effect's definition, as a small expression.

use bevy_mettle_attribute::Attribute;
use bevy_mettle_core::registry::{AnyAttribute, AnyCurve, Key};
use bevy_mettle_core::tag::{TagKey, Tags};
use serde::{Deserialize, Serialize};

use crate::code::AnyCalculation;

/// How much: a modifier's value, a duration or a period, from the spec.
///
/// A small expression: terms (a value, the level, a param, an attribute, a
/// calculation), a curve at any of them, and sums, products and clamps of
/// them. Unreal's `FGameplayEffectModifierMagnitude`, its kinds composed
/// rather than each with its own extras: a scalable float is a value times a
/// curve at the level; an attribute-based magnitude's coefficient and
/// additions, a product and a sum; an attribute curve, a curve at an
/// attribute.
///
/// ```
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] struct SpellPower;
/// let four = Magnitude::from(4.0);
/// let by_level = Magnitude::scaled(10.0, "fireball.damage");
/// let set_by_code = Magnitude::param("param.damage");
/// // Half the caster's bonus spell power, plus 10.
/// let power = Magnitude::source::<SpellPower>()
///     .bonus()
///     .times(0.5)
///     .plus(10.0);
/// ```
///
/// Anything more (division, conditions) is a [`Custom`](Self::Custom)
/// calculation, in code.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Magnitude {
    /// This value.
    Value(f32),
    /// The spec's level.
    Level,
    /// What the code applying it set, by a tag (`param.damage`: Unreal's
    /// `SetByCaller`); 0, with a warning, if it didn't.
    Param(TagKey),
    /// An attribute of the source's or the target's, captured as it says.
    Attribute(AttributeRead),
    /// Worked out by a calculation registered in code.
    Custom(Key<AnyCalculation>),
    /// A curve's value at `at`: the spec's level, unless it says.
    Curve {
        /// The curve.
        curve: Key<AnyCurve>,
        /// Where it's read: the spec's level, unless it says.
        #[serde(default = "level")]
        at: Box<Magnitude>,
    },
    /// These added up.
    Sum(Vec<Magnitude>),
    /// These multiplied together.
    Product(Vec<Magnitude>),
    /// `value`, kept within `min` and `max`.
    Clamp {
        /// What's kept within them.
        value: Box<Magnitude>,
        /// The least it can be, if there's a least.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min: Option<f32>,
        /// The most it can be, if there's a most.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        max: Option<f32>,
    },
}

#[expect(
    clippy::unnecessary_box_returns,
    reason = "serde's default for a boxed field"
)]
fn level() -> Box<Magnitude> {
    Box::new(Magnitude::Level)
}

impl Magnitude {
    /// `A` of the source's, its current value, captured as early as it can
    /// be: as the spec's made, or as it's applied. An [`AttributeRead`], to
    /// say more of how it's read; a magnitude wherever one's taken.
    #[must_use]
    pub fn source<A: Attribute>() -> AttributeRead {
        AttributeRead::new::<A>(Capture::Source)
    }

    /// `A` of the target's, its current value, captured as it's applied. An
    /// [`AttributeRead`], to say more of how it's read.
    #[must_use]
    pub fn target<A: Attribute>() -> AttributeRead {
        AttributeRead::new::<A>(Capture::Target)
    }

    /// What the code applying it set as `param`.
    #[must_use]
    pub fn param(param: impl Into<TagKey>) -> Self {
        Self::Param(param.into())
    }

    /// `curve` at the spec's level.
    #[must_use]
    pub fn curve(curve: impl Into<Key<AnyCurve>>) -> Self {
        Self::Curve {
            curve: curve.into(),
            at: level(),
        }
    }

    /// `curve` at `at`: an attribute's value, for diminishing returns.
    #[must_use]
    pub fn curve_at(curve: impl Into<Key<AnyCurve>>, at: impl Into<Magnitude>) -> Self {
        Self::Curve {
            curve: curve.into(),
            at: Box::new(at.into()),
        }
    }

    /// `base` times `curve` at the spec's level: Unreal's scalable float.
    #[must_use]
    pub fn scaled(base: f32, curve: impl Into<Key<AnyCurve>>) -> Self {
        Self::Product(vec![Self::Value(base), Self::curve(curve)])
    }

    /// It times `by`.
    #[must_use]
    pub fn times(self, by: impl Into<Magnitude>) -> Self {
        match self {
            Self::Product(mut terms) => {
                terms.push(by.into());
                Self::Product(terms)
            }
            this => Self::Product(vec![this, by.into()]),
        }
    }

    /// It plus `more`.
    #[must_use]
    pub fn plus(self, more: impl Into<Magnitude>) -> Self {
        match self {
            Self::Sum(mut terms) => {
                terms.push(more.into());
                Self::Sum(terms)
            }
            this => Self::Sum(vec![this, more.into()]),
        }
    }

    /// It, kept within `min` and `max`.
    #[must_use]
    pub fn clamp(self, min: Option<f32>, max: Option<f32>) -> Self {
        Self::Clamp {
            value: Box::new(self),
            min,
            max,
        }
    }
}

impl From<f32> for Magnitude {
    fn from(value: f32) -> Self {
        Self::Value(value)
    }
}

impl From<AttributeRead> for Magnitude {
    fn from(read: AttributeRead) -> Self {
        Self::Attribute(read)
    }
}

/// An attribute a magnitude reads: whose, which value, in what tags, and when.
///
/// Unreal's attribute capture definition with its attribute-based
/// magnitude's calculation type and tag filters. Read as the hit it's part
/// of is, as Unreal evaluates a capture with its spec's captured tags: in its
/// effect's tags and [`tags`](Self::tags), with its target and source and
/// what each owns, so modifiers counting for such hits count.
///
/// Started by [`Magnitude::source`] or [`Magnitude::target`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttributeRead {
    /// The attribute.
    pub attribute: Key<AnyAttribute>,
    /// Whose: the source's, unless it says.
    #[serde(default)]
    pub from: Capture,
    /// Which of its values: its current, unless it says.
    #[serde(default)]
    pub value: Read,
    /// Captured as early as it can be and kept (the source's as the spec's
    /// made, or as it's applied; the target's as it's applied), or read live,
    /// a lasting effect following it as it changes. Captured, unless it says.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub snapshot: bool,
    /// What it's read in beyond the hit it's part of (its effect's tags, its
    /// target and source and what each owns): the modifiers whose conditions
    /// hold in these tags count too.
    #[serde(default, skip_serializing_if = "Tags::is_empty")]
    pub tags: Tags,
}

fn yes() -> bool {
    true
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's skip_serializing_if passes a reference"
)]
fn is_yes(snapshot: &bool) -> bool {
    *snapshot
}

impl AttributeRead {
    pub(crate) fn new<A: Attribute>(from: Capture) -> Self {
        Self {
            attribute: Key::from_static(A::NAME),
            from,
            value: Read::Current,
            snapshot: true,
            tags: Tags::new(),
        }
    }

    /// Its base, not what modifiers make of it.
    #[must_use]
    pub fn base(mut self) -> Self {
        self.value = Read::Base;
        self
    }

    /// What modifiers add to its base: its current value less its base.
    #[must_use]
    pub fn bonus(mut self) -> Self {
        self.value = Read::Bonus;
        self
    }

    /// Read live: a lasting effect follows it as it changes.
    #[must_use]
    pub fn live(mut self) -> Self {
        self.snapshot = false;
        self
    }

    /// Read in `tag` too, beyond its effect's tags: the modifiers counting in
    /// it count.
    #[must_use]
    pub fn tagged(mut self, tag: impl Into<TagKey>) -> Self {
        self.tags.insert(tag);
        self
    }

    /// It times `by`.
    #[must_use]
    pub fn times(self, by: impl Into<Magnitude>) -> Magnitude {
        Magnitude::from(self).times(by)
    }

    /// It plus `more`.
    #[must_use]
    pub fn plus(self, more: impl Into<Magnitude>) -> Magnitude {
        Magnitude::from(self).plus(more)
    }
}

/// Whose attribute a magnitude reads: the source's or the target's.
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

/// Which of an attribute's values a magnitude reads: current, base or bonus.
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
pub enum Read {
    /// Its current value: its base, as modifiers make it.
    #[default]
    Current,
    /// Its base.
    Base,
    /// What modifiers add to its base: its current value less its base.
    Bonus,
}
