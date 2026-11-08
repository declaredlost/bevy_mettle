//! What's told as attributes change.

use std::fmt;
use std::marker::PhantomData;

use bevy::prelude::*;

use crate::Attribute;

/// Triggered as an actor's attribute `A` changes, with nothing in play.
///
/// A modifier came or went, its base was set, a tag it counts while came or
/// went. Told to observers of the actor (`commands.entity(hero).observe(..)`)
/// and to global ones (`On<AttributeChanged<Life>>`), as
/// [`AttributeSystems`](crate::AttributeSystems) runs. Values that only differ
/// for some reads (a hit's bonus) aren't told.
#[derive(EntityEvent)]
pub struct AttributeChanged<A: Attribute> {
    /// The actor.
    pub entity: Entity,
    /// What it was.
    pub old: f32,
    /// What it is.
    pub new: f32,
    attribute: PhantomData<fn() -> A>,
}

impl<A: Attribute> AttributeChanged<A> {
    pub(crate) fn new(entity: Entity, old: f32, new: f32) -> Self {
        Self {
            entity,
            old,
            new,
            attribute: PhantomData,
        }
    }
}

// By hand: derived, they'd ask `A` for them too, through its marker.
impl<A: Attribute> Clone for AttributeChanged<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Attribute> Copy for AttributeChanged<A> {}

impl<A: Attribute> PartialEq for AttributeChanged<A> {
    fn eq(&self, other: &Self) -> bool {
        (self.entity, self.old, self.new) == (other.entity, other.old, other.new)
    }
}

impl<A: Attribute> fmt::Debug for AttributeChanged<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AttributeChanged")
            .field("attribute", &A::NAME)
            .field("entity", &self.entity)
            .field("old", &self.old)
            .field("new", &self.new)
            .finish()
    }
}
