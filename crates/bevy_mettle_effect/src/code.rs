//! What effects run in code: executions, calculations, and routes with their
//! routers, each numbered as it's added.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld, Breakdown};
use bevy_mettle_attribute::registry::AttributeId;
use bevy_mettle_core::tag::{OwnedTags, TagKey};
use bevy_mettle_core::{Id, IdTarget, MettleError, Registry};

use crate::data::EffectDef;
use crate::{Effect, EffectId, EffectRegistry, EffectSpec};

/// What an execution or a calculation sees: the world, the target, the effect
/// and its spec.
///
/// The world read only, and how many stacks there are.
pub struct EffectContext<'a> {
    /// The world.
    pub world: &'a World,
    /// Who it's applied to.
    pub target: Entity,
    /// The effect: its number, name and tags.
    pub def: &'a EffectDef,
    /// What it's applied with.
    pub spec: &'a EffectSpec,
    /// How many stacks of it there are: 1 unless it stacks.
    pub stacks: u32,
}

impl EffectContext<'_> {
    /// The effect's number.
    #[must_use]
    pub fn effect(&self) -> EffectId {
        self.def.id
    }

    /// The spec's level.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.spec.level
    }

    /// What `param` was set to, if it was.
    #[must_use]
    pub fn param(&self, param: impl Into<TagKey>) -> Option<f32> {
        self.spec.param_value(param)
    }

    /// Who applied it, if anyone.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.spec.source
    }

    /// `entity`'s owned tags, if it has any.
    #[must_use]
    pub fn owned(&self, entity: Entity) -> Option<&OwnedTags> {
        self.world.get::<OwnedTags>(entity)
    }

    /// `A`'s number, if it's added: what a [`BaseChange`] names it by.
    #[must_use]
    pub fn attribute_id<A: Attribute>(&self) -> Option<AttributeId> {
        self.world.attribute_id::<A>()
    }

    /// `R`'s number, if it's added: what a [`BaseChange`] deals it by.
    #[must_use]
    pub fn route_id<R: Route>(&self) -> Option<RouteId> {
        self.world.get_resource::<RouteRegistry>()?.id_of::<R>()
    }

    /// `entity`'s `A` in `context`, if it has `A`.
    #[must_use]
    pub fn value<A: Attribute>(&self, entity: Entity, context: &AttributeContext) -> Option<f32> {
        self.world
            .value_at(entity, self.attribute_id::<A>()?, context)
    }

    /// `entity`'s `attribute` in `context`, if it has it.
    #[must_use]
    pub fn value_at(
        &self,
        entity: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<f32> {
        self.world.value_at(entity, attribute, context)
    }

    /// `entity`'s `A`'s modifiers in `context`, summed but not applied, if it
    /// has it: for a source's and a target's to be combined.
    #[must_use]
    pub fn breakdown<A: Attribute>(
        &self,
        entity: Entity,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        self.world
            .breakdown_at(entity, self.attribute_id::<A>()?, context)
    }

    /// `entity`'s `attribute`'s modifiers in `context`, summed but not applied,
    /// if it has it.
    #[must_use]
    pub fn breakdown_at(
        &self,
        entity: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        self.world.breakdown_at(entity, attribute, context)
    }
}

/// Whose base a change is to: the effect's target's, or its source's.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum Whose {
    /// The target's: damage taken.
    #[default]
    Target,
    /// The source's: life stolen back. None if the effect has no source.
    Source,
}

/// What a change changes: an attribute's base, or what's dealt a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Changes {
    /// This attribute's base.
    Attribute(AttributeId),
    /// What's dealt this route, for its router.
    Route(RouteId),
}

/// A change an execution or a router makes, as an instant modifier: to an
/// attribute's base, or dealt a route; its target's, unless it says.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseChange {
    /// What it changes.
    pub changes: Changes,
    /// How: only `Flat` is dealt a route.
    pub op: Operation,
    /// By how much.
    pub value: f32,
    /// Whose.
    pub on: Whose,
}

impl BaseChange {
    /// `op` of `value` to `attribute`'s base.
    #[must_use]
    pub fn new(attribute: AttributeId, op: Operation, value: f32) -> Self {
        Self {
            changes: Changes::Attribute(attribute),
            op,
            value,
            on: Whose::Target,
        }
    }

    /// `value` dealt `route`, for its router: what a hit deals, before shields
    /// and resistances.
    #[must_use]
    pub fn route(route: RouteId, value: f32) -> Self {
        Self {
            changes: Changes::Route(route),
            op: Operation::Flat,
            value,
            on: Whose::Target,
        }
    }

    /// To the source's, not the target's.
    #[must_use]
    pub fn on_source(mut self) -> Self {
        self.on = Whose::Source;
        self
    }
}

/// A route: an amount effects deal that no actor holds, routed into the
/// attributes they do as an effect executes.
///
/// Incoming damage, say: dealt by every hit, then routed through armour and
/// shields into life by one router, not each hit's own code. Derive it, and
/// add it with its router:
///
/// ```ignore
/// #[derive(Route)]
/// pub struct IncomingDamage;
///
/// app.add_route::<IncomingDamage>(route_damage);
/// ```
///
/// Data names it by name, `route: "incoming_damage"`, in the modifiers of an
/// effect that executes: instant, or periodic. It's dealt flat amounts, has no
/// base, limits or rounding, can't be read, and isn't replicated: what it's
/// routed into is.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a route",
    label = "not a route",
    note = "derive it: `#[derive(Route)]`"
)]
pub trait Route: Send + Sync + 'static {
    /// The name data names it by: `"incoming_damage"`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::Route;

/// A route's name in data: what an `Id<AnyRoute>` is, before it's resolved to
/// a [`RouteId`].
#[derive(IdTarget)]
#[id_target(name = "route")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyRoute {}

bevy_mettle_core::number! {
    /// A route's number in an app, given as it's added.
    pub struct RouteId;
}

/// An execution: a formula in code run as an effect executes, such as damage.
///
/// Run as an instant or periodic effect executes. Derive it, and add it with
/// what it runs:
///
/// ```ignore
/// #[derive(Execution)]
/// pub struct FireDamage;
///
/// app.add_execution::<FireDamage>(fire_damage);
/// ```
///
/// Data names it by name, `executions: ["fire_damage"]`; an app knows it by an
/// [`ExecutionId`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an execution",
    label = "not an execution",
    note = "derive it: `#[derive(Execution)]`"
)]
pub trait Execution: Send + Sync + 'static {
    /// The name data names it by: `"fire_damage"`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::Execution;

/// An execution's name in data: what an `Id<AnyExecution>` is, before it's
/// resolved to an [`ExecutionId`].
#[derive(IdTarget)]
#[id_target(name = "execution")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyExecution {}

bevy_mettle_core::number! {
    /// An execution's number in an app, given as it's added.
    pub struct ExecutionId;
}

/// A calculation: a magnitude worked out in code, such as how long a burn
/// lasts.
///
/// Derive it, and add it with what it works out:
///
/// ```ignore
/// #[derive(Calculation)]
/// pub struct BurnTime;
///
/// app.add_calculation::<BurnTime>(burn_time);
/// ```
///
/// Data names it by name, `Custom("burn_time")`; an app knows it by a
/// [`CalculationId`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a calculation",
    label = "not a calculation",
    note = "derive it: `#[derive(Calculation)]`"
)]
pub trait Calculation: Send + Sync + 'static {
    /// The name data names it by: `"burn_time"`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::Calculation;

/// A calculation's name in data: what an `Id<AnyCalculation>` is, before it's
/// resolved to a [`CalculationId`].
#[derive(IdTarget)]
#[id_target(name = "calculation")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCalculation {}

bevy_mettle_core::number! {
    /// A calculation's number in an app, given as it's added.
    pub struct CalculationId;
}

/// An execution: what it changes, worked out in code.
pub type ExecutionFn = Arc<dyn Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync>;

/// A calculation: a magnitude, worked out in code.
pub type CalculationFn = Arc<dyn Fn(&EffectContext) -> f32 + Send + Sync>;

/// A router: what a route's amount, all an execution dealt it, changes in the
/// attributes actors have.
pub type RouterFn = Arc<dyn Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync>;

/// The executions an app has, each numbered as it's added: what effects' data
/// names by name, and runs by number.
///
/// Added to with [`add_execution`](EffectApp::add_execution). Read as a
/// [`Registry`]: `get(id)`, `id(name)`, `id_of::<E>()`, `name(id)`, `iter()`.
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct ExecutionRegistry(Registry<ExecutionId, ExecutionFn>);

impl ExecutionRegistry {
    /// With the execution `E` running `execution`: for a registry built by
    /// hand.
    #[must_use]
    pub fn with<E: Execution>(
        mut self,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> Self {
        self.add::<E>(Arc::new(execution));
        self
    }

    /// Adds `E`, running `execution`, or replaces what it ran: its number.
    pub fn add<E: Execution>(&mut self, execution: ExecutionFn) -> ExecutionId {
        self.0.add::<E>(E::NAME, execution)
    }

    /// The number of the execution data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such execution.
    pub fn resolve(
        &self,
        name: &Id<AnyExecution>,
        within: Option<&str>,
    ) -> Result<ExecutionId, MettleError> {
        self.id(name.as_str())
            .ok_or_else(|| MettleError::unknown::<AnyExecution>(name.as_str(), within))
    }
}

/// The calculations an app has, each numbered as it's added: what effects'
/// data names by name, and works out by number.
///
/// Added to with [`add_calculation`](EffectApp::add_calculation). Read as a
/// [`Registry`].
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct CalculationRegistry(Registry<CalculationId, CalculationFn>);

impl CalculationRegistry {
    /// With the calculation `C` working out `calculation`: for a registry
    /// built by hand.
    #[must_use]
    pub fn with<C: Calculation>(
        mut self,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> Self {
        self.add::<C>(Arc::new(calculation));
        self
    }

    /// Adds `C`, working out `calculation`, or replaces what it worked out:
    /// its number.
    pub fn add<C: Calculation>(&mut self, calculation: CalculationFn) -> CalculationId {
        self.0.add::<C>(C::NAME, calculation)
    }

    /// The number of the calculation data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such calculation.
    pub fn resolve(
        &self,
        name: &Id<AnyCalculation>,
        within: Option<&str>,
    ) -> Result<CalculationId, MettleError> {
        self.id(name.as_str())
            .ok_or_else(|| MettleError::unknown::<AnyCalculation>(name.as_str(), within))
    }
}

/// The routes an app has, each with its router, numbered as it's added: what
/// effects' data deals by name, and executions by number.
///
/// Added to with [`add_route`](EffectApp::add_route). Read as a [`Registry`].
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct RouteRegistry(Registry<RouteId, RouterFn>);

impl RouteRegistry {
    /// With the route `R`, routed by `router`: for a registry built by hand.
    #[must_use]
    pub fn with<R: Route>(
        mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> Self {
        self.add::<R>(Arc::new(router));
        self
    }

    /// Adds `R`, routed by `router`, or replaces its router: its number.
    pub fn add<R: Route>(&mut self, router: RouterFn) -> RouteId {
        self.0.add::<R>(R::NAME, router)
    }

    /// The number of the route data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such route.
    pub fn resolve(
        &self,
        name: &Id<AnyRoute>,
        within: Option<&str>,
    ) -> Result<RouteId, MettleError> {
        self.id(name.as_str())
            .ok_or_else(|| MettleError::unknown::<AnyRoute>(name.as_str(), within))
    }
}

/// Adding effects' code to an app: executions, calculations and routes, each
/// numbered as it's added, and the effects code names.
pub trait EffectApp {
    /// Adds the execution `E`, running `execution`: a formula run as an effect
    /// naming it executes, such as damage.
    fn add_execution<E: Execution>(
        &mut self,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self;

    /// Adds the calculation `C`, working out `calculation`: a magnitude an
    /// effect naming it works out in code.
    fn add_calculation<C: Calculation>(
        &mut self,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> &mut Self;

    /// Adds the route `R`, routed by `router`: all an execution dealt it, into
    /// changes to attributes actors have.
    fn add_route<R: Route>(
        &mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self;

    /// Adds the effect `E` code names: content has to define it, or it's
    /// rejected.
    fn add_effect<E: Effect>(&mut self) -> &mut Self;
}

impl EffectApp for App {
    fn add_execution<E: Execution>(
        &mut self,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<ExecutionRegistry>()
            .add::<E>(Arc::new(execution));
        self
    }

    fn add_calculation<C: Calculation>(
        &mut self,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<CalculationRegistry>()
            .add::<C>(Arc::new(calculation));
        self
    }

    fn add_route<R: Route>(
        &mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<RouteRegistry>()
            .add::<R>(Arc::new(router));
        self
    }

    fn add_effect<E: Effect>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<EffectRegistry>()
            .name_type::<E>();
        self
    }
}
