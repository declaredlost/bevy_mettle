//! The attributes an app has: what data names, and what's checked.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::{Id, MettleError};

use crate::{AnyAttribute, Attribute, Base, Limit, Rounding};

/// What an app knows of one attribute: its rules, and how to reach its
/// [`Base`] on an actor without its type.
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

/// The attributes an app has, by id: added with
/// [`add_attribute`](AttributeApp::add_attribute). What data naming
/// attributes is checked against.
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

    /// Checks `attribute` is one there is, named within `within` if it's
    /// named in something.
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

    fn add<A: Attribute>(&mut self) -> bool {
        let attributes = Arc::make_mut(&mut self.attributes);
        if attributes.contains_key(&A::ID) {
            return false;
        }
        attributes.insert(A::ID, AttributeInfo::of::<A>());
        true
    }
}

/// Adding attributes to an app.
pub trait AttributeApp {
    /// Adds attribute `A`: data can name it, and actors with its [`Base`]
    /// have its [`Current`](crate::Current) kept and its changes told.
    /// Adds [`AttributePlugin`](crate::AttributePlugin) if it isn't yet.
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
            warn!("attribute `{}` is added twice", A::NAME);
            return self;
        }
        let schedule = self
            .world()
            .resource::<crate::plugin::AttributeSchedule>()
            .0;
        self.add_systems(
            schedule,
            (
                crate::state::sync_base::<A>.in_set(crate::plugin::Phase::Sync),
                crate::state::publish::<A>.in_set(crate::plugin::Phase::Publish),
            ),
        )
    }
}
