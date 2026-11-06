//! Behaviours: what runs an ability, in code.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::{Id, IdTarget};

/// A behaviour: the component in code that runs an ability, by name.
///
/// Named by an `Id<Behaviour>`.
#[derive(IdTarget)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Behaviour {}

/// How a behaviour's component is put on an ability entity and taken off.
#[derive(Clone, Copy)]
pub(crate) struct BehaviourFns {
    pub(crate) insert: fn(&mut EntityWorldMut),
    pub(crate) remove: fn(&mut EntityWorldMut),
}

/// The behaviours there are, by name: what abilities' data is checked against.
///
/// Added with [`add_behaviour`](BehaviourApp::add_behaviour).
#[derive(Resource, Clone, Default)]
pub struct Behaviours(pub(crate) Arc<HashMap<Id<Behaviour>, BehaviourFns>>);

impl Behaviours {
    /// Whether `behaviour` is there.
    #[must_use]
    pub fn contains(&self, behaviour: &Id<Behaviour>) -> bool {
        self.0.contains_key(behaviour)
    }
}

/// Adding behaviours to an app: a component registered under a name.
pub trait BehaviourApp {
    /// Adds the behaviour `name`: as an ability run by it activates, `B` is put
    /// on the ability's entity, as it was by default, for the game's systems to
    /// run; it's taken off as the ability ends.
    fn add_behaviour<B: Component + Default>(
        &mut self,
        name: impl Into<Id<Behaviour>>,
    ) -> &mut Self;
}

impl BehaviourApp for App {
    fn add_behaviour<B: Component + Default>(
        &mut self,
        name: impl Into<Id<Behaviour>>,
    ) -> &mut Self {
        let mut behaviours = self.world_mut().get_resource_or_init::<Behaviours>();
        Arc::make_mut(&mut behaviours.0).insert(
            name.into(),
            BehaviourFns {
                insert: |entity| {
                    entity.insert(B::default());
                },
                remove: |entity| {
                    entity.remove::<B>();
                },
            },
        );
        self
    }
}
