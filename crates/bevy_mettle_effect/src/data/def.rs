//! Effects as an app has them: checked, what their data names resolved to
//! numbers.

use bevy_mettle_attribute::modifier::{Operation, Per};
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::registry::{AnyEffect, AttributeId, CurveId, EffectId, Key};
use bevy_mettle_core::tag::{TagKey, Tags};

use super::{EffectDuration, Period, Read, Side, Stacking};
use crate::code::{CalculationId, Changes, ExecutionId, RouteId};

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
    pub name: Key<AnyEffect>,
    /// What it's labelled.
    pub tags: Tags,
    /// How long it lasts.
    pub duration: EffectDuration<MagnitudeDef>,
    /// How often it executes, if it does as it lasts.
    pub period: Option<Period<MagnitudeDef>>,
    /// What it changes.
    pub modifiers: Vec<ModifierDef>,
    /// What it captures: the slots its magnitudes' attributes, and its
    /// executions', calculations' and routers' captures, are read from.
    pub captures: Vec<CaptureDef>,
    /// The executions it runs, each with what it captures and routes into.
    pub executions: Vec<ExecutionDef>,
    /// The routes it routes into, each with the slots of what its router
    /// captures: those its modifiers name, and its executions declare.
    pub routes: Vec<(RouteId, Vec<usize>)>,
    /// What its target, the owner, holds while it counts.
    pub owner_tags: Tags,
    /// The cues it plays.
    pub cues: Tags,
    /// What has to hold for it to apply.
    pub require: Option<Condition>,
    /// What has to hold for it to count.
    pub ongoing: Option<Condition>,
    /// What ends it as soon as it holds.
    pub end_if: Option<Condition>,
    /// The effects it ends as it applies.
    pub ends: Tags,
    /// The effects it keeps from applying while it lasts.
    pub blocks: Tags,
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

/// An execution as an effect runs it: its number, captures and routes.
///
/// What it captures as slots in the effect's
/// [`captures`](EffectDef::captures), and the routes it declares it routes
/// into, numbered.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutionDef {
    /// Its number.
    pub id: ExecutionId,
    /// The slots of what it captures.
    pub reads: Vec<usize>,
    /// The routes it routes into: any other it routes into is dropped.
    pub routes: Vec<RouteId>,
}

/// A modifier as an app has it: what it changes by number.
#[derive(Debug, Clone, PartialEq)]
pub struct ModifierDef {
    /// What it changes: an attribute, or what's routed into a route.
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
/// Its attributes are capture slots in the effect's
/// [`captures`](EffectDef::captures); its constants folded.
#[derive(Debug, Clone, PartialEq)]
pub enum MagnitudeDef {
    /// This value.
    Value(f32),
    /// The spec's level.
    Level,
    /// What the code applying it set, by a tag.
    Param(TagKey),
    /// The effect's capture in this slot of its
    /// [`captures`](EffectDef::captures).
    Attribute(usize),
    /// Worked out by a calculation registered in code, with the slots of what
    /// it captures.
    Custom(CalculationId, Vec<usize>),
    /// A curve's value at `at`.
    Curve {
        /// The curve.
        curve: CurveId,
        /// Where it's read.
        at: Box<MagnitudeDef>,
    },
    /// These added up.
    Sum(Vec<MagnitudeDef>),
    /// These multiplied together.
    Product(Vec<MagnitudeDef>),
    /// `value`, kept within `min` and `max`.
    Clamp {
        /// What's kept within them.
        value: Box<MagnitudeDef>,
        /// The least it can be, if there's a least.
        min: Option<f32>,
        /// The most it can be, if there's a most.
        max: Option<f32>,
    },
}

/// A capture as an app has it: what an effect's magnitudes and code read.
///
/// An attribute the effect captures, for its magnitudes, executions and
/// calculations.
///
/// One slot each, in [`EffectDef::captures`]: the same read, named twice, is
/// one capture.
#[derive(Debug, Clone, PartialEq)]
pub struct CaptureDef {
    /// The attribute.
    pub attribute: AttributeId,
    /// Whose.
    pub from: Side,
    /// Which of its values.
    pub value: Read,
    /// Captured as early as it can be and kept, or read live.
    pub snapshot: bool,
    /// What it's read in beyond its effect's tags: its own.
    pub tags: Tags,
    /// What it's read in, all of it: its effect's tags and its own, as a
    /// hit's attributes are, so modifiers counting for such hits count.
    pub read_in: Tags,
}
