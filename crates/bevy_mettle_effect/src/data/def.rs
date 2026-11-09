//! Effects as an app has them: checked, what their data names resolved to
//! numbers.

use bevy_mettle_attribute::modifier::{Operation, Per};
use bevy_mettle_attribute::registry::AttributeId;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::curve::CurveId;
use bevy_mettle_core::registry::Name;
use bevy_mettle_core::tag::{TagKey, Tags};

use super::{AnyEffect, Capture, EffectDuration, Period, Stacking};
use crate::code::{CalculationId, Changes, ExecutionId};
use crate::registry::EffectId;

/// An effect as an app has it: its data checked, and what it names numbered.
///
/// Its [`EffectData`](super::EffectData) checked, the effects, curves,
/// attributes, routes, executions and calculations it names resolved to their
/// numbers.
///
/// What [`EffectRegistry`](crate::EffectRegistry) holds, and an active effect
/// runs by.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectDef {
    /// Its number.
    pub id: EffectId,
    /// Its name.
    pub name: Name<AnyEffect>,
    /// What it's labelled.
    pub tags: Tags,
    /// How long it lasts.
    pub duration: EffectDuration<MagnitudeDef>,
    /// How often it executes, if it does as it lasts.
    pub period: Option<Period<MagnitudeDef>>,
    /// What it changes.
    pub modifiers: Vec<ModifierDef>,
    /// The executions it runs.
    pub executions: Vec<ExecutionId>,
    /// What its target owns while it counts.
    pub grants: Tags,
    /// The cues it plays.
    pub cues: Tags,
    /// What has to hold for it to apply.
    pub require: Option<Condition>,
    /// What has to hold for it to count.
    pub ongoing: Option<Condition>,
    /// What takes it off as soon as it holds.
    pub remove_if: Option<Condition>,
    /// The effects it takes off as it applies.
    pub removes: Tags,
    /// The effects it keeps from applying.
    pub immunity: Tags,
    /// How it stacks.
    pub stacking: Option<Stacking<EffectId>>,
    /// The effects it applies as it applies.
    pub on_apply: Vec<EffectId>,
    /// The effects it applies as it's taken off.
    pub on_remove: Vec<EffectId>,
    /// The effects it applies as it runs out.
    pub on_expire: Vec<EffectId>,
}

impl EffectDef {
    /// Whether it lasts at all.
    #[must_use]
    pub fn lasts(&self) -> bool {
        !matches!(self.duration, EffectDuration::Instant)
    }
}

/// A modifier as an app has it: what it changes by number.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifierDef {
    /// What it changes: an attribute, or what's dealt a route.
    pub changes: Changes,
    /// How.
    pub op: Operation,
    /// By how much.
    pub value: MagnitudeDef,
    /// When it counts, and how much.
    pub condition: Option<Condition>,
    /// That it counts once for each whole so much of an attribute.
    pub per: Option<Per>,
}

/// How much, as an app has it: a [`Magnitude`](super::Magnitude), its names
/// numbered.
///
/// What the magnitude names resolved to their numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum MagnitudeDef {
    /// This value.
    Value(f32),
    /// `base` times `curve` at the spec's level.
    Scaled {
        /// What the curve's value is multiplied by.
        base: f32,
        /// The curve.
        curve: CurveId,
    },
    /// What the code applying it set, by a tag.
    Param(TagKey),
    /// An attribute of the source's or the target's, as it is when the
    /// effect's applied, times `coefficient`.
    Attribute {
        /// The attribute.
        attribute: AttributeId,
        /// Whose.
        from: Capture,
        /// What it's multiplied by.
        coefficient: f32,
    },
    /// Worked out by a calculation registered in code.
    Custom(CalculationId),
}
