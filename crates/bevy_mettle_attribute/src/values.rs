//! An attribute's base and current values, on its actor.

use std::fmt;
use std::marker::PhantomData;

use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy::reflect::utility::GenericTypePathCell;

use crate::Attribute;
use crate::registry::AttributeState;

/// On an actor: attribute `A`'s base, before modifiers: what the game sets.
///
/// What the game and effects set (what's left of life, a level gained). Having
/// it is having the attribute; it brings [`Current<A>`], at this base until
/// it's first worked out. Taking it off takes the attribute away, and its
/// `Current<A>` with it.
#[derive(Component, Reflect)]
#[reflect(Component, Default, type_path = false)]
#[require(Current<A>, AttributeState)]
#[component(on_insert = seed::<A>)]
pub struct Base<A: Attribute> {
    value: f32,
    #[reflect(ignore)]
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

/// Starts `Current<A>` at the base inserted, until it's first worked out.
fn seed<A: Attribute>(mut world: DeferredWorld, context: HookContext) {
    let Some(base) = world.get::<Base<A>>(context.entity).map(Base::get) else {
        return;
    };
    if let Some(mut current) = world.get_mut::<Current<A>>(context.entity)
        && !current.worked_out
    {
        current.bypass_change_detection().value = base;
    }
}

/// On an actor: attribute `A`'s current value, modifiers and limits applied.
///
/// Every modifier that counts with nothing in play (GAS's current value).
/// Mettle's to write, as mettle publishes values
/// ([`MettleSet::Publish`](bevy_mettle_core::MettleSet::Publish)),
/// and only when it changes: `Changed<Current<A>>` is it changing. For a read
/// in context (a hit's tags), see
/// [`AttributeReader`](crate::read::AttributeReader).
#[derive(Component, Reflect)]
#[reflect(Component, Default, type_path = false)]
pub struct Current<A: Attribute> {
    value: f32,
    /// Whether it's been worked out yet: until then, it's the base.
    worked_out: bool,
    #[reflect(ignore)]
    attribute: PhantomData<fn() -> A>,
}

impl<A: Attribute> Default for Current<A> {
    fn default() -> Self {
        Self {
            value: A::BASE,
            worked_out: false,
            attribute: PhantomData,
        }
    }
}

impl<A: Attribute> Current<A> {
    /// Its value: its base's, until it's first worked out.
    #[must_use]
    pub fn get(&self) -> f32 {
        self.value
    }

    /// Its value, if it's been worked out.
    pub(crate) fn worked_out(&self) -> Option<f32> {
        self.worked_out.then_some(self.value)
    }

    /// Its value worked out: what it was, if it had been.
    pub(crate) fn replace(&mut self, value: f32) -> Option<f32> {
        let was = self.worked_out();
        self.value = value;
        self.worked_out = true;
        was
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
        write!(f, "Base<{}>({})", A::KEY, self.value)
    }
}

impl<A: Attribute> Clone for Current<A> {
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            worked_out: self.worked_out,
            attribute: PhantomData,
        }
    }
}

impl<A: Attribute> PartialEq for Current<A> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && self.worked_out == other.worked_out
    }
}

impl<A: Attribute> fmt::Debug for Current<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Current<{}>({})", A::KEY, self.get())
    }
}

/// Implements [`TypePath`] for `$ty<A>`, named by `A`'s name.
macro_rules! type_path {
    ($ty:ident) => {
        /// Named by its attribute's name: `bevy_mettle_attribute::Base<life>`.
        impl<A: Attribute> TypePath for $ty<A> {
            fn type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self, _>(|| {
                    format!("bevy_mettle_attribute::{}<{}>", stringify!($ty), A::KEY)
                })
            }

            fn short_type_path() -> &'static str {
                static CELL: GenericTypePathCell = GenericTypePathCell::new();
                CELL.get_or_insert::<Self, _>(|| format!("{}<{}>", stringify!($ty), A::KEY))
            }

            fn type_ident() -> Option<&'static str> {
                Some(stringify!($ty))
            }

            fn crate_name() -> Option<&'static str> {
                Some("bevy_mettle_attribute")
            }

            fn module_path() -> Option<&'static str> {
                Some("bevy_mettle_attribute")
            }
        }
    };
}

type_path!(Base);
type_path!(Current);

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
