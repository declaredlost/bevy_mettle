//! The untyped layer: attributes by number, their rules, and actors' state.
//!
//! What data names them by, and what's told as values change.

mod events;
mod rules;
pub(crate) mod state;

use bevy::prelude::*;
use bevy_mettle_core::Error;
use bevy_mettle_core::registry::{AnyAttribute, AttributeId, Name, Registry};

pub use self::events::AttributeChanged;
pub use self::rules::{Bound, Limit, OnLimit, Rounding};
pub use self::state::AttributeState;
use crate::{Attribute, Base};

/// What an app knows of one attribute: its number, name, rules and base.
///
/// It reaches the attribute's [`Base`] on an actor without its type.
#[derive(Clone)]
pub struct AttributeInfo {
    /// Its number.
    pub id: AttributeId,
    /// The name data names it by, and what's shown for it.
    pub name: &'static str,
    /// What its base starts at.
    pub base: f32,
    /// The least it can be, once the attribute it names, if any, is added.
    pub min: Option<Bound>,
    /// The most it can be, as `min`.
    pub max: Option<Bound>,
    /// How it's rounded, if it's a whole number.
    pub rounding: Option<Rounding>,
    /// What its base does as a limit that's another attribute moves.
    pub on_limit: OnLimit,
    /// Its limits as its type says them, to be resolved.
    limits: [Option<Limit>; 2],
    pub(crate) get_base: fn(&World, Entity) -> Option<f32>,
    pub(crate) set_base: fn(&mut World, Entity, f32) -> bool,
}

impl AttributeInfo {
    fn of<A: Attribute>(id: AttributeId) -> Self {
        Self {
            id,
            name: A::NAME,
            base: A::BASE,
            min: None,
            max: None,
            rounding: A::ROUNDING,
            on_limit: A::ON_LIMIT,
            limits: [A::MIN, A::MAX],
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

    /// Whether a limit of it is another attribute: its base can follow it.
    #[must_use]
    pub fn follows(&self) -> bool {
        self.limits
            .iter()
            .any(|limit| matches!(limit, Some(Limit::Attribute(_))))
    }
}

/// The attributes an app has: info by number, and numbers by type and by name.
///
/// Added to with [`add_attribute`](AttributeApp::add_attribute), which gives
/// each the next number, as Bevy numbers components: the same app adds them in
/// the same order, on a server and its clients alike. Read as a [`Registry`]:
/// `get(id)`, `id(name)`, `id_of::<A>()`, `name(id)`, `iter()`.
#[derive(Resource, Deref, Clone, Default)]
pub struct AttributeRegistry(Registry<AnyAttribute, AttributeInfo>);

impl AttributeRegistry {
    /// The number of the attribute data names `name`, named within `within` if
    /// it's named in something.
    ///
    /// # Errors
    ///
    /// If there's no such attribute.
    pub fn resolve(
        &self,
        name: &Name<AnyAttribute>,
        within: Option<&str>,
    ) -> Result<AttributeId, Error> {
        self.id(name.as_str())
            .ok_or_else(|| Error::unknown::<AnyAttribute>(name.as_str(), within))
    }

    /// Adds `A`: its number, and whether it's new.
    ///
    /// # Panics
    ///
    /// If another type has its name: data couldn't tell them apart.
    pub(crate) fn add<A: Attribute>(&mut self) -> (AttributeId, bool) {
        if let Some(id) = self.id_of::<A>() {
            return (id, false);
        }
        let next = AttributeId::new(u32::try_from(self.len()).expect("fewer than 2^32 attributes"));
        let id = self.0.add::<A>(A::NAME, AttributeInfo::of::<A>(next));
        self.bind();
        (id, true)
    }

    /// Resolves every attribute's limits that name an attribute added since.
    fn bind(&mut self) {
        let numbers = self.0.clone();
        let bound = |limit: Option<Limit>| match limit? {
            Limit::Value(value) => Some(Bound::Value(value)),
            Limit::Attribute(type_id) => numbers.id_of_type(type_id).map(Bound::Attribute),
        };
        for (_, info) in self.0.iter_mut() {
            let [min, max] = info.limits;
            info.min = bound(min);
            info.max = bound(max);
        }
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
        let (id, added) = self
            .world_mut()
            .resource_mut::<AttributeRegistry>()
            .add::<A>();
        if !added {
            return self;
        }
        // Only one whose limits are attributes has a base to move as they do.
        let follows = self
            .world()
            .resource::<AttributeRegistry>()
            .get(id)
            .is_some_and(AttributeInfo::follows);
        if follows {
            let schedule = self
                .world()
                .resource::<crate::plugin::AttributeSchedule>()
                .0;
            self.add_systems(
                schedule,
                crate::registry::state::follow_limits::<A>.in_set(crate::plugin::Phase::Follow),
            );
        }
        self.register_type::<crate::Base<A>>()
            .register_type::<crate::Current<A>>()
            .register_type::<crate::authority::Served<A>>()
            .add_observer(crate::registry::state::remove_base::<A>);
        let schedule = self
            .world()
            .resource::<crate::plugin::AttributeSchedule>()
            .0;
        self.add_systems(
            schedule,
            (
                (
                    crate::registry::state::sync_base::<A>,
                    crate::authority::sync_served::<A>,
                )
                    .in_set(crate::plugin::Phase::Sync),
                (
                    crate::registry::state::publish::<A>,
                    crate::authority::serve::<A>,
                )
                    .chain()
                    .in_set(crate::plugin::Phase::Publish),
            ),
        )
    }
}
