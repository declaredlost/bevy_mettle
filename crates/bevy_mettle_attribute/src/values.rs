//! An attribute's base and current values, on its actor.

use std::fmt;
use std::marker::PhantomData;

use bevy::prelude::*;

use crate::Attribute;
use crate::registry::AttributeState;

/// On an actor: attribute `A`'s base, before modifiers: what the game sets.
///
/// What the game and effects set (what's left of life, a level gained). Having
/// it is having the attribute; it brings [`Current<A>`].
#[derive(Component)]
#[require(Current<A>, AttributeState)]
pub struct Base<A: Attribute> {
    value: f32,
    attribute: PhantomData<fn() -> A>,
}

impl<A: Attribute> Default for Base<A> {
    /// Where it starts.
    fn default() -> Self {
        Self::new(A::BASE)
    }
}

impl<A: Attribute> Base<A> {
    /// At `value`.
    #[must_use]
    pub fn new(value: f32) -> Self {
        Self {
            value,
            attribute: PhantomData,
        }
    }

    /// Its value.
    #[must_use]
    pub fn get(&self) -> f32 {
        self.value
    }

    /// Sets it.
    pub fn set(&mut self, value: f32) {
        self.value = value;
    }

    /// Adds `value` to it.
    pub fn add(&mut self, value: f32) {
        self.value += value;
    }
}

/// On an actor: attribute `A`'s current value, modifiers and limits applied.
///
/// Every modifier that counts with nothing in play (GAS's current value).
/// Mettle's to write, as [`AttributeSystems`](crate::AttributeSystems) runs,
/// and only when it changes: `Changed<Current<A>>` is it changing. For a read
/// in context (a hit's tags), see
/// [`AttributeReader`](crate::read::AttributeReader).
#[derive(Component)]
pub struct Current<A: Attribute> {
    /// `None` until it's first worked out.
    value: Option<f32>,
    attribute: PhantomData<fn() -> A>,
}

impl<A: Attribute> Default for Current<A> {
    fn default() -> Self {
        Self {
            value: None,
            attribute: PhantomData,
        }
    }
}

impl<A: Attribute> Current<A> {
    /// Its value: where it starts, until it's first worked out.
    #[must_use]
    pub fn get(&self) -> f32 {
        self.value.unwrap_or(A::BASE)
    }

    /// Its value, if it's been worked out.
    pub(crate) fn worked_out(&self) -> Option<f32> {
        self.value
    }

    pub(crate) fn replace(&mut self, value: f32) {
        self.value = Some(value);
    }
}

impl<A: Attribute> Clone for Base<A> {
    fn clone(&self) -> Self {
        Self::new(self.value)
    }
}

impl<A: Attribute> PartialEq for Base<A> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<A: Attribute> fmt::Debug for Base<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Base<{}>({})", A::NAME, self.value)
    }
}

impl<A: Attribute> Clone for Current<A> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            attribute: PhantomData,
        }
    }
}

impl<A: Attribute> PartialEq for Current<A> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<A: Attribute> fmt::Debug for Current<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Current<{}>({})", A::NAME, self.get())
    }
}

/// Written as its value: what a server replicates, the base being its to
/// decide.
impl<A: Attribute> serde::Serialize for Base<A> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_f32(self.value)
    }
}

impl<'de, A: Attribute> serde::Deserialize<'de> for Base<A> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        f32::deserialize(deserializer).map(Self::new)
    }
}
