//! What effects run in code: executions, calculations, and routes with routers.
//!
//! Each numbered as it's added.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld, Breakdown};
use bevy_mettle_core::Error;
use bevy_mettle_core::registry::{AttributeId, EffectId, Key, Kind, Name, Registry};
use bevy_mettle_core::tag::{OwnedTags, TagKey};

use crate::context::{ContextData, EffectContext};
use crate::data::{AttributeRead, Capture, EffectDef};
use crate::{Effect, EffectRegistry, EffectSpec};

/// What an execution or a calculation sees: the world, target, effect and spec.
///
/// The world read only, and how many stacks there are.
pub struct ExecutionContext<'a> {
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
    /// What it captured, by its [`captures`](EffectDef::captures)' slots:
    /// none for one read live, or not taken yet.
    pub captured: &'a [Option<f32>],
    /// The slots of what the execution or calculation running declared it
    /// captures: none for anything else.
    pub reads: &'a [usize],
}

impl ExecutionContext<'_> {
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

    /// What it carries: who applied it, what struck, where.
    #[must_use]
    pub fn context(&self) -> &EffectContext {
        &self.spec.context
    }

    /// Who applied it, if anyone.
    #[must_use]
    pub fn source(&self) -> Option<Entity> {
        self.spec.context.source
    }

    /// `A` of `from`'s, as the execution or calculation running declared it
    /// captures it: captured, or read live.
    ///
    /// `None` if it didn't declare it, warned of once: what isn't declared
    /// can't be captured.
    #[must_use]
    pub fn captured<A: Attribute>(&self, from: Capture) -> Option<f32> {
        let attribute = self.attribute_id::<A>()?;
        let slot = self.reads.iter().copied().find(|&slot| {
            self.def
                .captures
                .get(slot)
                .is_some_and(|capture| capture.attribute == attribute && capture.from == from)
        });
        let Some(slot) = slot else {
            if crate::context::first_warning(format!(
                "undeclared {} {from:?} {}",
                self.def.name,
                A::NAME
            )) {
                warn!(
                    "effect `{}`'s code reads the {from:?}'s `{}`, which it doesn't declare it \
                     captures (`source({})` or `target({})` on its derive): none",
                    self.def.name,
                    A::NAME,
                    A::NAME,
                    A::NAME
                );
            }
            return None;
        };
        Some(crate::engine::captured(self, slot))
    }

    /// What the game added of type `C`, if it did.
    #[must_use]
    pub fn get<C: ContextData>(&self) -> Option<&C> {
        self.spec.context.get::<C>()
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

/// A change an execution or a router makes, as an instant modifier.
///
/// To an attribute's base, or dealt a route; its target's, unless it says.
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

/// A route: an amount effects deal that no actor holds, routed into attributes.
///
/// Routed into the attributes actors do hold, as an effect executes.
///
/// Incoming damage, say: dealt by every hit, then routed through armour and
/// shields into life by one router, not each hit's own code. Derive it, route
/// it with [`RouteFn`], and add it:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(min = 0.0)] struct Shield;
/// # #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// #[derive(Route)]
/// pub struct IncomingDamage;
///
/// impl RouteFn for IncomingDamage {
///     fn route(&self, cx: &ExecutionContext, amount: f32) -> Vec<BaseChange> {
///         let (Some(shield), Some(life)) = (cx.attribute_id::<Shield>(), cx.attribute_id::<Life>())
///         else {
///             return Vec::new();
///         };
///         // The shield takes what it can, and life the rest.
///         let held = cx.value::<Shield>(cx.target, &AttributeContext::new()).unwrap_or_default();
///         let absorbed = amount.min(held);
///         vec![
///             BaseChange::new(shield, Operation::Flat, -absorbed),
///             BaseChange::new(life, Operation::Flat, absorbed - amount),
///         ]
///     }
/// }
///
/// App::new().add_route(IncomingDamage);
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

/// The route kind: what data names and deals, and a [`RouteId`] numbers.
///
/// A route's name in data is a `Name<AnyRoute>`, until it's resolved to a
/// [`RouteId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyRoute {}

/// A route's number in an app, given as it's added: what executions deal by.
pub type RouteId = Key<AnyRoute>;

/// An execution: a formula in code run as an effect executes, such as damage.
///
/// Run as an instant or periodic effect executes. Derive it, run it with
/// [`ExecutionFn`], and add it:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// # #[derive(Attribute)] struct SpellPower;
/// # #[derive(Attribute)] struct Resistance;
/// /// The spec's damage, more with the caster's spell power, less with the
/// /// target's fire resistance.
/// #[derive(Execution)]
/// #[execution(source(SpellPower), target(Resistance, tagged("damage.fire")))]
/// pub struct FireDamage;
///
/// impl ExecutionFn for FireDamage {
///     fn execute(&self, cx: &ExecutionContext) -> Vec<BaseChange> {
///         let damage = cx.param("param.damage").unwrap_or_default();
///         let power = cx.captured::<SpellPower>(Capture::Source).unwrap_or_default();
///         let resisted = cx.captured::<Resistance>(Capture::Target).unwrap_or_default();
///         let dealt = damage * (1.0 + power / 100.0) * (1.0 - resisted);
///         let Some(life) = cx.attribute_id::<Life>() else {
///             return Vec::new();
///         };
///         vec![BaseChange::new(life, Operation::Flat, -dealt)]
///     }
/// }
///
/// App::new().add_execution(FireDamage);
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

    /// The attributes it captures, read with
    /// [`captured`](ExecutionContext::captured): its derive's
    /// `#[execution(source(A, ..), target(B, ..))]`.
    #[must_use]
    fn captures() -> Vec<AttributeRead> {
        Vec::new()
    }
}

pub use bevy_mettle_macros::Execution;

/// The execution kind: what data names, and an [`ExecutionId`] numbers.
///
/// An execution's name in data is a `Name<AnyExecution>`, until it's resolved
/// to an [`ExecutionId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyExecution {}

/// An execution's number in an app, given as it's added, as effects know it.
pub type ExecutionId = Key<AnyExecution>;

/// A calculation: a magnitude worked out in code, such as a burn's duration.
///
/// Derive it, work it out with [`CalculationFn`], and add it:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] struct SpellPower;
/// /// Two seconds, and one more for every `per` spell power.
/// #[derive(Calculation)]
/// #[calculation(source(SpellPower))]
/// pub struct BurnTime {
///     per: f32,
/// }
///
/// impl CalculationFn for BurnTime {
///     fn calculate(&self, cx: &ExecutionContext) -> f32 {
///         let power = cx
///             .captured::<SpellPower>(Capture::Source)
///             .unwrap_or_default();
///         2.0 + power / self.per
///     }
/// }
///
/// App::new().add_calculation(BurnTime { per: 25.0 });
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

    /// The attributes it captures, read with
    /// [`captured`](ExecutionContext::captured): its derive's
    /// `#[calculation(source(A, ..), target(B, ..))]`.
    #[must_use]
    fn captures() -> Vec<AttributeRead> {
        Vec::new()
    }
}

pub use bevy_mettle_macros::Calculation;

/// The calculation kind: what data names, and a [`CalculationId`] numbers.
///
/// A calculation's name in data is a `Name<AnyCalculation>`, until it's
/// resolved to a [`CalculationId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCalculation {}

/// A calculation's number in an app, given as it's added, as effects know it.
pub type CalculationId = Key<AnyCalculation>;

/// What an [`Execution`] runs: what it changes, worked out from its context.
///
/// Unreal's `Execute`. On the execution's type, with what it's set up with
/// (`&self`); it reads the world, never changes it: what it changes, it
/// returns, so a client's prediction of it can be undone.
pub trait ExecutionFn: Send + Sync + 'static {
    /// What it changes, worked out from `cx`.
    fn execute(&self, cx: &ExecutionContext) -> Vec<BaseChange>;
}

/// What a [`Calculation`] works out: a magnitude, from its context.
///
/// Unreal's `CalculateBaseMagnitude`. On the calculation's type, with what
/// it's set up with (`&self`).
pub trait CalculationFn: Send + Sync + 'static {
    /// The magnitude, worked out from `cx`.
    fn calculate(&self, cx: &ExecutionContext) -> f32;
}

/// What a [`Route`] routes: all an execution dealt it, as attribute changes.
///
/// Changes to the attributes actors have.
///
/// On the route's type, with what it's set up with (`&self`).
pub trait RouteFn: Send + Sync + 'static {
    /// What `amount`, all `cx`'s execution dealt the route, changes.
    fn route(&self, cx: &ExecutionContext, amount: f32) -> Vec<BaseChange>;
}

/// Code an effect runs, as a registry holds it, and what it captures.
pub struct Code<F: ?Sized> {
    run: Arc<F>,
    captures: Arc<[AttributeRead]>,
}

impl<F: ?Sized> Code<F> {
    fn new(run: Arc<F>, captures: Vec<AttributeRead>) -> Self {
        Self {
            run,
            captures: captures.into(),
        }
    }

    /// What it runs.
    #[must_use]
    pub fn run(&self) -> &F {
        &self.run
    }

    /// What it captures.
    #[must_use]
    pub fn captures(&self) -> &[AttributeRead] {
        &self.captures
    }
}

impl<F: ?Sized> Clone for Code<F> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
            captures: Arc::clone(&self.captures),
        }
    }
}

/// The executions an app has: named by data, and run by number.
///
/// Each numbered as it's added.
///
/// Added to with [`add_execution`](EffectApp::add_execution). Read as a
/// [`Registry`]: `get(id)`, `id(name)`, `id_of::<E>()`, `name(id)`, `iter()`.
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct ExecutionRegistry(Registry<AnyExecution, Code<dyn ExecutionFn>>);

impl ExecutionRegistry {
    /// With `execution`: for a registry built by hand.
    #[must_use]
    pub fn with<E: Execution + ExecutionFn>(mut self, execution: E) -> Self {
        self.add(execution);
        self
    }

    /// Adds `execution`, or replaces what its type added: its number.
    pub fn add<E: Execution + ExecutionFn>(&mut self, execution: E) -> ExecutionId {
        self.0
            .add::<E>(E::NAME, Code::new(Arc::new(execution), E::captures()))
    }

    /// The number of the execution data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such execution.
    pub fn resolve(
        &self,
        name: &Name<AnyExecution>,
        within: Option<&str>,
    ) -> Result<ExecutionId, Error> {
        self.id(name.as_str())
            .ok_or_else(|| Error::unknown::<AnyExecution>(name.as_str(), within))
    }
}

/// The calculations an app has: named by data, worked out by number.
///
/// Each numbered as it's added.
///
/// Added to with [`add_calculation`](EffectApp::add_calculation). Read as a
/// [`Registry`].
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct CalculationRegistry(Registry<AnyCalculation, Code<dyn CalculationFn>>);

impl CalculationRegistry {
    /// With `calculation`: for a registry built by hand.
    #[must_use]
    pub fn with<C: Calculation + CalculationFn>(mut self, calculation: C) -> Self {
        self.add(calculation);
        self
    }

    /// Adds `calculation`, or replaces what its type added: its number.
    pub fn add<C: Calculation + CalculationFn>(&mut self, calculation: C) -> CalculationId {
        self.0
            .add::<C>(C::NAME, Code::new(Arc::new(calculation), C::captures()))
    }

    /// The number of the calculation data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such calculation.
    pub fn resolve(
        &self,
        name: &Name<AnyCalculation>,
        within: Option<&str>,
    ) -> Result<CalculationId, Error> {
        self.id(name.as_str())
            .ok_or_else(|| Error::unknown::<AnyCalculation>(name.as_str(), within))
    }
}

/// The routes an app has, with their routers: named by data, dealt by number.
///
/// Each numbered as it's added. Effects' data deals a route by name, and
/// executions by number.
///
/// Added to with [`add_route`](EffectApp::add_route). Read as a [`Registry`].
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct RouteRegistry(Registry<AnyRoute, Arc<dyn RouteFn>>);

impl RouteRegistry {
    /// With `route`: for a registry built by hand.
    #[must_use]
    pub fn with<R: Route + RouteFn>(mut self, route: R) -> Self {
        self.add(route);
        self
    }

    /// Adds `route`, or replaces what its type added: its number.
    pub fn add<R: Route + RouteFn>(&mut self, route: R) -> RouteId {
        self.0.add::<R>(R::NAME, Arc::new(route))
    }

    /// The number of the route data names `name`, named within `within`.
    ///
    /// # Errors
    ///
    /// If there's no such route.
    pub fn resolve(&self, name: &Name<AnyRoute>, within: Option<&str>) -> Result<RouteId, Error> {
        self.id(name.as_str())
            .ok_or_else(|| Error::unknown::<AnyRoute>(name.as_str(), within))
    }
}

/// Adding effects' code to an app: executions, calculations, routes, effects.
///
/// Executions, calculations and routes, each numbered as it's added, and the
/// effects code names.
pub trait EffectApp {
    /// Adds `execution`: a formula run as an effect naming it executes, such
    /// as damage.
    fn add_execution<E: Execution + ExecutionFn>(&mut self, execution: E) -> &mut Self;

    /// Adds `calculation`: a magnitude an effect naming it works out in code.
    fn add_calculation<C: Calculation + CalculationFn>(&mut self, calculation: C) -> &mut Self;

    /// Adds `route`: all an execution dealt it, routed into changes to
    /// attributes actors have.
    fn add_route<R: Route + RouteFn>(&mut self, route: R) -> &mut Self;

    /// Adds the effect `E` code names: content has to define it, or it's
    /// rejected.
    fn add_effect<E: Effect>(&mut self) -> &mut Self;
}

impl EffectApp for App {
    fn add_execution<E: Execution + ExecutionFn>(&mut self, execution: E) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<ExecutionRegistry>()
            .add(execution);
        self
    }

    fn add_calculation<C: Calculation + CalculationFn>(&mut self, calculation: C) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<CalculationRegistry>()
            .add(calculation);
        self
    }

    fn add_route<R: Route + RouteFn>(&mut self, route: R) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<RouteRegistry>()
            .add(route);
        self
    }

    fn add_effect<E: Effect>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<EffectRegistry>()
            .name_type::<E>();
        self
    }
}
