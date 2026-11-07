//! The untyped layer: attributes by id, their rules, and each actor's state.
//!
//! What data names them by, and what's told as values change.

mod events;
mod rules;
pub(crate) mod state;

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::{Id, MettleError};

pub use self::events::AttributeChanged;
pub use self::rules::{AnyAttribute, Limit, Rounding};
pub use self::state::AttributeState;
use crate::{Attribute, Base};

/// What an app knows of one attribute: its rules, and access to its base.
///
/// It reaches the attribute's [`Base`] on an actor without its type.
#[derive(Clone)]
pub struct AttributeInfo {
    /// Its id.
    pub id: Id<AnyAttribute>,
    /// What its base starts at.
    pub base: f32,
    /// The least it can be.
    pub min: Option<Limit>,
    /// The most it can be.
    pub max: Option<Limit>,
    /// How it's rounded, if it's a whole number.
    pub rounding: Option<Rounding>,
    /// The type it is: two types can't share a name.
    pub(crate) type_id: TypeId,
    /// The type's name, for when two would share one.
    pub(crate) type_name: &'static str,
    pub(crate) get_base: fn(&World, Entity) -> Option<f32>,
    pub(crate) set_base: fn(&mut World, Entity, f32) -> bool,
}

impl AttributeInfo {
    fn of<A: Attribute>() -> Self {
        Self {
            id: A::ID,
            base: A::BASE,
            min: A::MIN,
            max: A::MAX,
            rounding: A::ROUNDING,
            type_id: TypeId::of::<A>(),
            type_name: std::any::type_name::<A>(),
            get_base: |world, actor| Some(world.get::<Base<A>>(actor)?.get()),
            set_base: |world, actor, value| {
                let Some(mut base) = world.get_mut::<Base<A>>(actor) else {
                    return false;
                };
                base.set(value);
                true
            },
        }
    }
}

/// The attributes an app has, by id: what data naming them is checked against.
///
/// Added with [`add_attribute`](AttributeApp::add_attribute).
#[derive(Resource, Clone, Default)]
pub struct AttributeRegistry {
    attributes: Arc<HashMap<Id<AnyAttribute>, AttributeInfo>>,
}

impl AttributeRegistry {
    /// The attribute `id`, if there is one.
    #[must_use]
    pub fn get(&self, id: &Id<AnyAttribute>) -> Option<&AttributeInfo> {
        self.attributes.get(id)
    }

    /// Every attribute.
    pub fn iter(&self) -> impl Iterator<Item = &AttributeInfo> {
        self.attributes.values()
    }

    /// Checks `attribute` is one there is, named within `within` if it's named
    /// in something.
    ///
    /// # Errors
    ///
    /// If it isn't.
    pub fn check(
        &self,
        attribute: &Id<AnyAttribute>,
        within: Option<&str>,
    ) -> Result<(), MettleError> {
        if self.get(attribute).is_some() {
            Ok(())
        } else {
            Err(MettleError::unknown::<AnyAttribute>(
                attribute.as_str(),
                within,
            ))
        }
    }

    /// Adds `A`: whether it's new.
    ///
    /// # Panics
    ///
    /// If another type has its name: data couldn't tell them apart.
    pub(crate) fn add<A: Attribute>(&mut self) -> bool {
        let attributes = Arc::make_mut(&mut self.attributes);
        if let Some(known) = attributes.get(&A::ID) {
            assert!(
                known.type_id == TypeId::of::<A>(),
                "attributes `{}` and `{}` are both named `{}`",
                known.type_name,
                std::any::type_name::<A>(),
                A::NAME,
            );
            return false;
        }
        attributes.insert(A::ID, AttributeInfo::of::<A>());
        true
    }
}

/// Adding attributes to an app: `app.add_attribute::<Life>()`.
pub trait AttributeApp {
    /// Adds attribute `A`: data can name it, and actors with its [`Base`] have
    /// its [`Current`](crate::Current) kept and its changes told; both are
    /// registered for reflection. Adds
    /// [`AttributePlugin`](crate::AttributePlugin) if it isn't yet.
    ///
    /// # Panics
    ///
    /// If another attribute type has the same name.
    fn add_attribute<A: Attribute>(&mut self) -> &mut Self;
}

impl AttributeApp for App {
    fn add_attribute<A: Attribute>(&mut self) -> &mut Self {
        if !self.is_plugin_added::<crate::AttributePlugin>() {
            self.add_plugins(crate::AttributePlugin::default());
        }
        let added = self
            .world_mut()
            .resource_mut::<AttributeRegistry>()
            .add::<A>();
        if !added {
            return self;
        }
        self.register_type::<crate::Base<A>>()
            .register_type::<crate::Current<A>>()
            .add_observer(crate::registry::state::remove_base::<A>);
        let schedule = self
            .world()
            .resource::<crate::plugin::AttributeSchedule>()
            .0;
        self.add_systems(
            schedule,
            (
                crate::registry::state::sync_base::<A>.in_set(crate::plugin::Phase::Sync),
                crate::registry::state::publish::<A>.in_set(crate::plugin::Phase::Publish),
            ),
        )
    }
}
