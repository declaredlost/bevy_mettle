//! Behaviours: what runs an ability, in code, and the input it takes.

use std::any::{Any, TypeId};
use std::sync::Arc;

use bevy::ecs::system::IntoSystem;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_core::registry::{Id, Kind, Registry};
pub use bevy_mettle_macros::Behaviour;

/// A behaviour: the component in code that runs an ability.
///
/// Put on the ability's entity as it activates, as it was by default, for the
/// game's systems to run; taken off as it ends. Its fields are run state, the
/// server's to keep: what the client chose is its [`Input`](Self::Input).
/// Derive it:
///
/// ```ignore
/// #[derive(Component, Default, Behaviour)]
/// #[behaviour(name = "cast_fireball", input = Aim)]
/// struct CastFireball { timer: f32 }
/// ```
///
/// | Setting | Meaning | Unless it says |
/// |---|---|---|
/// | `name = ".."` | The name data names it by | its type's, in `snake_case` |
/// | `input = Type` | The input its abilities are activated with | none: [`NoInput`] |
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a behaviour",
    label = "not a behaviour",
    note = "derive it: `#[derive(Component, Default, Behaviour)]`"
)]
pub trait Behaviour: Component + Default {
    /// The name data names it by, and what's shown for it: `"cast_fireball"`.
    /// Its number is the app's: [`BehaviourRegistry`].
    const NAME: &'static str;

    /// The input its abilities are activated with, put on the ability next to
    /// it: what the client chose (an aim, a target). [`NoInput`] if none.
    type Input: Component + Clone;
}

/// The [`Behaviour::Input`] of a behaviour that takes none: nothing to send.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoInput;

/// The behaviour kind: what data names, and a [`BehaviourId`] numbers.
///
/// A behaviour's name in data is a `Key<AnyBehaviour>`, until it's resolved
/// to a [`BehaviourId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyBehaviour {}

/// A behaviour's number in an app: what abilities and their requests use.
///
/// Given as it's added.
pub type BehaviourId = Id<AnyBehaviour>;

/// What an input check is given: the input, and whose it is.
///
/// The check is a system: `fn in_range(In(check): In<InputCheck<Aim>>, ..) ->
/// bool`, so it can query anything, a spatial query for line of sight too.
#[derive(Debug, Clone)]
pub struct InputCheck<I> {
    /// The actor whose ability it's for.
    pub actor: Entity,
    /// The granted ability.
    pub ability: Entity,
    /// The input.
    pub input: I,
}

/// A behaviour's input type.
#[derive(Clone, Copy)]
pub(crate) struct InputInfo {
    pub(crate) type_id: TypeId,
    pub(crate) name: &'static str,
}

/// How a behaviour's component and input are put on an ability, and taken off.
///
/// Put on an ability entity, and taken off: what [`BehaviourRegistry`] holds
/// for each.
#[derive(Clone, Copy)]
pub struct BehaviourFns {
    pub(crate) insert: fn(&mut EntityWorldMut),
    pub(crate) remove: fn(&mut EntityWorldMut),
    /// Its input, if it takes one.
    pub(crate) input: Option<InputInfo>,
}

/// The behaviours there are: each by number, and numbers by type and name.
///
/// What abilities' data is checked against.
///
/// Added to with [`add_behaviour`](BehaviourApp::add_behaviour), which gives
/// each the next number, as Bevy numbers components. Read as a [`Registry`]:
/// `id(name)`, `id_of::<B>()`, `name(id)`.
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct BehaviourRegistry(Registry<AnyBehaviour, BehaviourFns>);

impl BehaviourRegistry {
    /// The name of the input `behaviour` takes, if it takes one.
    #[must_use]
    pub fn input(&self, behaviour: BehaviourId) -> Option<&'static str> {
        self.0.get(behaviour)?.input.map(|input| input.name)
    }

    /// Adds `B`, made by `fns`, or replaces what's there: its number.
    fn add<B: Behaviour>(&mut self, fns: BehaviourFns) -> BehaviourId {
        self.0.add::<B>(B::NAME, fns)
    }
}

/// An input check, its input given as `Any`.
pub(crate) type CheckFn = Arc<dyn Fn(&mut World, Entity, Entity, &dyn Any) -> bool + Send + Sync>;

/// The checks inputs are put through, by input type.
#[derive(Resource, Clone, Default)]
pub(crate) struct InputChecks(pub(crate) HashMap<TypeId, CheckFn>);

/// Adding behaviours to an app, and checks of their input.
pub trait BehaviourApp {
    /// Adds the behaviour `B`, by its name: as an ability run by it activates,
    /// `B` is put on the ability's entity, as it was by default, with the
    /// input it was activated with; both are taken off as it ends.
    fn add_behaviour<B: Behaviour>(&mut self) -> &mut Self;

    /// Checks each `I` given to an ability with `check`, a system, before it's
    /// taken: at activation, wherever the ability activates; while it's
    /// active, where it's decided (a server, or a world on its own). Input
    /// that doesn't hold rejects the activation.
    fn check_input<I, M>(
        &mut self,
        check: impl IntoSystem<In<InputCheck<I>>, bool, M> + Copy + Send + Sync + 'static,
    ) -> &mut Self
    where
        I: Component + Clone;
}

impl BehaviourApp for App {
    fn add_behaviour<B: Behaviour>(&mut self) -> &mut Self {
        let input = (TypeId::of::<B::Input>() != TypeId::of::<NoInput>()).then(|| InputInfo {
            type_id: TypeId::of::<B::Input>(),
            name: std::any::type_name::<B::Input>(),
        });
        if input.is_some() {
            crate::net::add_input::<B::Input>(self);
        }
        let mut behaviours = self.world_mut().get_resource_or_init::<BehaviourRegistry>();
        behaviours.add::<B>(BehaviourFns {
            insert: |entity| {
                entity.insert(B::default());
            },
            remove: |entity| {
                entity.remove::<(B, B::Input)>();
            },
            input,
        });
        self
    }

    fn check_input<I, M>(
        &mut self,
        check: impl IntoSystem<In<InputCheck<I>>, bool, M> + Copy + Send + Sync + 'static,
    ) -> &mut Self
    where
        I: Component + Clone,
    {
        let run: CheckFn = Arc::new(move |world, actor, ability, input| {
            let Some(input) = input.downcast_ref::<I>() else {
                return false;
            };
            let given = InputCheck {
                actor,
                ability,
                input: input.clone(),
            };
            world
                .run_system_cached_with(check, given)
                .unwrap_or_else(|error| {
                    warn!("an input check failed to run: {error}");
                    false
                })
        });
        self.world_mut()
            .get_resource_or_init::<InputChecks>()
            .0
            .insert(TypeId::of::<I>(), run);
        self
    }
}
