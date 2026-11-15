//! How applying an effect again stacks.

use bevy_mettle_core::registry::{AnyEffect, Key};

use crate::Effect;
use serde::{Deserialize, Serialize};

/// How an effect applied again onto its target stacks, up to a limit.
///
/// Into one active effect, its modifiers' amounts counted once per stack, up to
/// `limit`.
///
/// A stack is a count on the effect as first applied: its spec (level, source,
/// context) and what it captured stay as they were, and applying again adds a
/// stack and, as `reapply` says, its time; the new spec goes only to the
/// effects it chains. Stack `by` source to keep each source's applications
/// apart. At the limit, applying again applies `overflow` instead of the
/// effect's own `on_apply`.
///
/// Its overflow effects are named (`E`, a `Key<AnyEffect>`) in data, and
/// numbered (an [`EffectId`](bevy_mettle_core::registry::EffectId)) once
/// checked.
///
/// ```
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Effect)] struct Detonate;
/// let poison = Stacking::up_to(5).by(StackBy::Source).overflow::<Detonate>();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stacking<E = Key<AnyEffect>> {
    /// Whose stacks are one: everyone's on the target, unless it says.
    #[serde(default)]
    pub by: StackBy,
    /// The most stacks: more than 0.
    pub limit: u32,
    /// What applying again does to the time left, a new stack or at the
    /// limit: refreshes it, unless it says.
    #[serde(default)]
    pub reapply: Reapply,
    /// Whether applying again, a new stack or at the limit, starts the period
    /// over.
    #[serde(default)]
    pub reset_period: bool,
    /// What running out of time does: takes every stack off, unless it says.
    #[serde(default)]
    pub expire: Expire,
    /// Effects applied to the target in its place when it's at its limit.
    #[serde(default = "Vec::new", skip_serializing_if = "Vec::is_empty")]
    pub overflow: Vec<E>,
}

/// Whose applications of an effect stack: everyone's, or each source's apart.
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
pub enum StackBy {
    /// Everyone's on the target: one stack of poison, whoever applied it.
    #[default]
    Target,
    /// Each source's apart: each caster's own stacks.
    Source,
}

/// What a new stack does to the time an effect has left: refresh or keep it.
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
pub enum Reapply {
    /// Starts it over.
    #[default]
    Refresh,
    /// Leaves it.
    Keep,
}

/// What running out of time does to a stacked effect: clear, drop one, refresh.
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
pub enum Expire {
    /// Takes every stack off.
    #[default]
    ClearAll,
    /// Takes one stack off, and starts the time over for the rest.
    RemoveOne,
    /// Starts the time over, keeping every stack.
    Refresh,
}

impl Stacking {
    /// Up to `limit` stacks, everyone's one, each refreshing the time, all
    /// taken off as it runs out.
    #[must_use]
    pub fn up_to(limit: u32) -> Self {
        Self {
            by: StackBy::Target,
            limit,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::ClearAll,
            overflow: Vec::new(),
        }
    }

    /// Whose stacks are one, as `by` says.
    #[must_use]
    pub fn by(mut self, by: StackBy) -> Self {
        self.by = by;
        self
    }

    /// Applying again doing to the time left as `reapply` says.
    #[must_use]
    pub fn reapply(mut self, reapply: Reapply) -> Self {
        self.reapply = reapply;
        self
    }

    /// Applying again starting the period over.
    #[must_use]
    pub fn reset_period(mut self) -> Self {
        self.reset_period = true;
        self
    }

    /// Running out of time doing as `expire` says.
    #[must_use]
    pub fn expire(mut self, expire: Expire) -> Self {
        self.expire = expire;
        self
    }

    /// Applying `E` in its place at its limit, too.
    #[must_use]
    pub fn overflow<E: Effect>(mut self) -> Self {
        self.overflow.push(Key::of::<E>());
        self
    }
}
