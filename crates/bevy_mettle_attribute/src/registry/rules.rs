//! What attributes are named by, and what their rules are made of.

use std::any::TypeId;

use bevy_mettle_core::IdTarget;
use serde::{Deserialize, Serialize};

use crate::Attribute;

/// An attribute's name in data: what an `Id<AnyAttribute>` is, before it's
/// resolved to an [`AttributeId`].
#[derive(IdTarget)]
#[id_target(name = "attribute")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyAttribute {}

bevy_mettle_core::number! {
    /// An attribute's number in an app: dense, given as it's added, and what
    /// state, modifiers and the network know it by.
    ///
    /// As Bevy's `ComponentId` is to a component: look one up from its type
    /// with [`AttributeRegistry::id`](super::AttributeRegistry::id) or
    /// [`AttributeWorld::attribute_id`](crate::read::AttributeWorld::attribute_id),
    /// and its name and rules with
    /// [`AttributeRegistry::get`](super::AttributeRegistry::get).
    pub struct AttributeId;
}

/// What an attribute is kept within, as its type says: a value, or another
/// attribute's value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Limit {
    /// This value.
    Value(f32),
    /// The current value of the attribute of this type, as the actor has it,
    /// read in the same context.
    Attribute(TypeId),
}

impl Limit {
    /// `A`'s current value.
    #[must_use]
    pub const fn attribute<A: Attribute>() -> Self {
        Self::Attribute(TypeId::of::<A>())
    }
}

/// A [`Limit`] as an app has it: its attribute resolved to its number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Bound {
    /// This value.
    Value(f32),
    /// This attribute's current value, as the actor has it, read in the same
    /// context.
    Attribute(AttributeId),
}

/// What an attribute's base does as a limit that's another attribute moves:
/// life as its maximum drops, or rises.
///
/// Only the world that decides attributes moves it: a server, or a game on
/// its own. A client sees what the server serves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum OnLimit {
    /// Kept within its limits: life above its new maximum comes down to it,
    /// and stays there as the maximum rises again.
    #[default]
    Clamp,
    /// Kept at the same fraction of its maximum: half life stays half as the
    /// maximum moves, either way. Then kept within its limits.
    Ratio,
}

/// How a whole-number attribute is rounded: down, up, or to the nearest.
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
pub enum Rounding {
    /// Towards negative infinity: 2.9 is 2.
    #[default]
    Down,
    /// Towards positive infinity: 2.1 is 3.
    Up,
    /// To the nearest, halves away from 0: 2.5 is 3.
    Nearest,
}

impl Rounding {
    pub(crate) fn round(self, value: f32) -> f32 {
        match self {
            Self::Down => value.floor(),
            Self::Up => value.ceil(),
            Self::Nearest => value.round(),
        }
    }
}
