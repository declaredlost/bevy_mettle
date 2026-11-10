//! What effects run in code: executions, calculations, and routes with routers.
//!
//! Each numbered as it's added.

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld, Breakdown};
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::registry::{
    AnyAttribute, AttributeId, EffectId, Id, Key, Keyed, Kind, Ref, Registry,
};
use bevy_mettle_core::tag::{OwnedTags, TagKey};

use crate::context::{ContextData, EffectContext};
use crate::data::{AttributeRead, EffectDef, Side};
use crate::{Effect, EffectRegistry, EffectSpec};

/// What effect code sees: the world, target, effect, spec and its captures.
///
/// What an execution, a calculation or a router reads. Attributes are read
/// through [`captured`](Self::captured), as declared on the code's derive:
/// snapshotted, checked and predicted.
pub struct ExecutionContext<'a> {
    /// The world, read only: the raw escape hatch, for what a capture can't
    /// read (a third entity's). What's read here isn't snapshotted, checked
    /// or predicted.
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
    /// The slots of what the code running declared it captures: none for
    /// anything else.
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

    /// `A` of `from`'s, as the code running declared it captures it:
    /// captured, or read live.
    ///
    /// `None` if it didn't declare it, warned of once: what isn't declared
    /// can't be captured.
    #[must_use]
    pub fn captured<A: Attribute>(&self, from: Side) -> Option<f32> {
        let attribute = self.world.attribute_id::<A>()?;
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
                A::KEY
            )) {
                warn!(
                    "effect `{}`'s code reads the {from:?}'s `{}`, which it doesn't declare it \
                     captures (`source({})` or `target({})` on its derive): none",
                    self.def.name,
                    A::KEY,
                    A::KEY,
                    A::KEY
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

    /// `entity`'s `A`'s modifiers in `context`, summed but not applied, if it
    /// has it: for a source's and a target's to be combined.
    ///
    /// The one read past captures: read now, never snapshotted or predicted.
    #[must_use]
    pub fn breakdown<A: Attribute>(
        &self,
        entity: Entity,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        self.world.breakdown::<A>(entity, context)
    }
}

/// What a change changes: an attribute's base, or what's routed into a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Changes {
    /// This attribute's base.
    Attribute(AttributeId),
    /// What's routed into this route, for its router.
    Route(RouteId),
}

/// A change an execution or a router makes, as an instant modifier.
///
/// To an attribute's base, or routed into a route; its target's, unless it
/// says. Named by type, `BaseChange::of::<Life>(..)` or
/// `BaseChange::route::<IncomingDamage>(..)`, it's numbered as it's applied.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseChange {
    /// What it changes: by type until it's applied, then by number.
    names: Names,
    /// How: only `Flat` is routed into a route.
    pub op: Operation,
    /// By how much.
    pub value: f32,
    /// Whose: the target's (damage taken), unless it says; the source's
    /// for life stolen back, none if the effect has no source.
    pub on: Side,
}

/// What a change changes, as it was named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Names {
    /// An attribute's base.
    Attribute(Ref<AnyAttribute>),
    /// What's routed into a route.
    Route(Ref<AnyRoute>),
}

impl BaseChange {
    /// `op` of `value` to `A`'s base.
    #[must_use]
    pub fn of<A: Attribute>(op: Operation, value: f32) -> Self {
        Self::named(Names::Attribute(Ref::of::<A>()), op, value)
    }

    /// `value` routed into `R`, for its router: a hit's damage, before
    /// shields and resistances.
    ///
    /// An execution routes only into what it declares,
    /// `#[execution(routes(R))]`: into anything else, it's dropped, warned of
    /// once.
    #[must_use]
    pub fn route<R: Route>(value: f32) -> Self {
        Self::named(Names::Route(Ref::of::<R>()), Operation::Flat, value)
    }

    /// `op` of `value` to what `changes` names, by number: what an effect's
    /// data changes.
    pub(crate) fn changing(changes: Changes, op: Operation, value: f32) -> Self {
        let names = match changes {
            Changes::Attribute(id) => Names::Attribute(Ref::Id(id)),
            Changes::Route(id) => Names::Route(Ref::Id(id)),
        };
        Self::named(names, op, value)
    }

    /// To the source's, not the target's.
    #[must_use]
    pub fn on_source(mut self) -> Self {
        self.on = Side::Source;
        self
    }

    /// What it changes, by number: `None` until it's applied.
    #[must_use]
    pub fn changes(&self) -> Option<Changes> {
        match self.names {
            Names::Attribute(named) => named.id().map(Changes::Attribute),
            Names::Route(named) => named.id().map(Changes::Route),
        }
    }

    /// Numbered as `world` numbers what it names; `None`, warned of once, if
    /// that isn't added.
    pub(crate) fn numbered(self, world: &World) -> Option<Self> {
        let (changes, what, label) = match self.names {
            Names::Attribute(named) => (
                world
                    .get_resource::<AttributeRegistry>()
                    .and_then(|attributes| attributes.find(named))
                    .map(Changes::Attribute),
                "attribute",
                named.label(),
            ),
            Names::Route(named) => (
                world
                    .get_resource::<RouteRegistry>()
                    .and_then(|routes| routes.find(named))
                    .map(Changes::Route),
                "route",
                named.label(),
            ),
        };
        if let Some(changes) = changes {
            let mut numbered = Self::changing(changes, self.op, self.value);
            numbered.on = self.on;
            return Some(numbered);
        }
        if crate::context::first_warning(format!("unadded {what} {label}")) {
            warn!("a change to the {what} `{label}`, which isn't added: dropped");
        }
        None
    }

    /// Named so.
    fn named(names: Names, op: Operation, value: f32) -> Self {
        Self {
            names,
            op,
            value,
            on: Side::Target,
        }
    }
}

/// A route: an amount no actor holds, routed on into attributes.
///
/// Its router turns it into changes to the attributes actors do hold, as an
/// effect executes.
///
/// Incoming damage, say: every hit routes into it, then one router sends it
/// through armour and shields into life, not each hit's own code. Derive it,
/// route it with [`RouteFn`], and add it:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Attribute)] #[attribute(min = 0.0)] struct Shield;
/// # #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// /// The target's shield, read as each hit's routed.
/// #[derive(Route)]
/// #[route(target(Shield, live))]
/// pub struct IncomingDamage;
///
/// impl RouteFn for IncomingDamage {
///     fn route(&self, cx: &ExecutionContext, amount: f32) -> Vec<BaseChange> {
///         // The shield takes what it can, and life the rest.
///         let held = cx.captured::<Shield>(Side::Target).unwrap_or_default();
///         let absorbed = amount.min(held);
///         vec![
///             BaseChange::of::<Shield>(Operation::Flat, -absorbed),
///             BaseChange::of::<Life>(Operation::Flat, absorbed - amount),
///         ]
///     }
/// }
///
/// App::new().add_route(IncomingDamage);
/// ```
///
/// Data names it by name, `route: "incoming_damage"`, in the modifiers of an
/// effect that executes: instant, or periodic. An execution names it by type,
/// declaring it, `#[execution(routes(IncomingDamage))]`, and routing into it,
/// [`BaseChange::route`]. It takes flat amounts, has no base, limits or
/// rounding, can't be read, and isn't replicated: what it's routed into is.
/// What its router reads it captures, as an execution does: every effect that
/// routes into it, by a modifier or an execution's declaration, captures it.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a route",
    label = "not a route",
    note = "derive it: `#[derive(Route)]`"
)]
pub trait Route: Keyed<Kind = AnyRoute> + Send + Sync + 'static {
    /// The attributes its router captures, read with
    /// [`captured`](ExecutionContext::captured): its derive's
    /// `#[route(source(A, ..), target(B, ..))]`.
    #[must_use]
    fn captures() -> Vec<AttributeRead> {
        Vec::new()
    }
}

pub use bevy_mettle_macros::Route;

/// The route kind: what data names and routes into, and a [`RouteId`] numbers.
///
/// A route's name in data is a `Key<AnyRoute>`, until it's resolved to a
/// [`RouteId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyRoute {}

/// A route's number in an app, given as it's added, as effects know it.
pub type RouteId = Id<AnyRoute>;

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
///         let power = cx.captured::<SpellPower>(Side::Source).unwrap_or_default();
///         let resisted = cx.captured::<Resistance>(Side::Target).unwrap_or_default();
///         let dealt = damage * (1.0 + power / 100.0) * (1.0 - resisted);
///         vec![BaseChange::of::<Life>(Operation::Flat, -dealt)]
///     }
/// }
///
/// App::new().add_execution(FireDamage);
/// ```
///
/// An execution routing into a [`Route`] declares it,
/// `#[execution(routes(IncomingDamage))]`, and returns
/// [`BaseChange::route`]: its effect captures what that route's router does.
///
/// Data names it by name, `executions: ["fire_damage"]`; an app knows it by an
/// [`ExecutionId`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an execution",
    label = "not an execution",
    note = "derive it: `#[derive(Execution)]`"
)]
pub trait Execution: Keyed<Kind = AnyExecution> + Send + Sync + 'static {
    /// The attributes it captures, read with
    /// [`captured`](ExecutionContext::captured): its derive's
    /// `#[execution(source(A, ..), target(B, ..))]`.
    #[must_use]
    fn captures() -> Vec<AttributeRead> {
        Vec::new()
    }

    /// The routes it routes into, with [`BaseChange::route`]: its derive's
    /// `#[execution(routes(R, ..))]`.
    #[must_use]
    fn routes() -> Vec<Key<AnyRoute>> {
        Vec::new()
    }
}

pub use bevy_mettle_macros::Execution;

/// The execution kind: what data names, and an [`ExecutionId`] numbers.
///
/// An execution's name in data is a `Key<AnyExecution>`, until it's resolved
/// to an [`ExecutionId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyExecution {}

/// An execution's number in an app, given as it's added, as effects know it.
pub type ExecutionId = Id<AnyExecution>;

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
///         let power =
///             cx.captured::<SpellPower>(Side::Source).unwrap_or_default();
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
pub trait Calculation: Keyed<Kind = AnyCalculation> + Send + Sync + 'static {
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
/// A calculation's name in data is a `Key<AnyCalculation>`, until it's
/// resolved to a [`CalculationId`].
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyCalculation {}

/// A calculation's number in an app, given as it's added, as effects know it.
pub type CalculationId = Id<AnyCalculation>;

/// What an [`Execution`] runs: what it changes, worked out from its context.
///
/// Unreal's `Execute`. On the execution's type, with what it's set up with
/// (`&self`); it reads the world, never changes it: what it changes, it
/// returns, so a client's prediction of it can be undone. It changes
/// attributes' bases, and routes into the routes it declares.
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

/// What a [`Route`] routes: all an effect routed into it, as attribute changes.
///
/// Changes to the attributes actors have.
///
/// On the route's type, with what it's set up with (`&self`).
pub trait RouteFn: Send + Sync + 'static {
    /// What `amount`, all `cx`'s effect routed into the route, changes.
    fn route(&self, cx: &ExecutionContext, amount: f32) -> Vec<BaseChange>;
}

/// Code an effect runs, as a registry holds it, and what it captures.
///
/// And, an execution's, the routes it routes into.
pub struct Code<F: ?Sized> {
    run: Arc<F>,
    captures: Arc<[AttributeRead]>,
    routes: Arc<[Key<AnyRoute>]>,
}

impl<F: ?Sized> Code<F> {
    fn new(run: Arc<F>, captures: Vec<AttributeRead>, routes: Vec<Key<AnyRoute>>) -> Self {
        Self {
            run,
            captures: captures.into(),
            routes: routes.into(),
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

    /// The routes it routes into: an execution's, as it declares them.
    #[must_use]
    pub fn routes(&self) -> &[Key<AnyRoute>] {
        &self.routes
    }
}

impl<F: ?Sized> Clone for Code<F> {
    fn clone(&self) -> Self {
        Self {
            run: Arc::clone(&self.run),
            captures: Arc::clone(&self.captures),
            routes: Arc::clone(&self.routes),
        }
    }
}

/// The code of one kind an app added: named by data, and run by number.
///
/// Each numbered as it's added, with what it captures. Its kinds are
/// [`ExecutionRegistry`], [`CalculationRegistry`] and [`RouteRegistry`],
/// added to with [`EffectApp`]. Read as a [`Registry`]: `get(id)`,
/// `id(name)`, `id_of::<E>()`, `name(id)`, `iter()`.
#[derive(Resource, Deref)]
pub struct CodeRegistry<K: Kind, F: ?Sized>(Registry<K, Code<F>>);

/// The executions an app has, run by number: added with
/// [`add_execution`](EffectApp::add_execution).
pub type ExecutionRegistry = CodeRegistry<AnyExecution, dyn ExecutionFn>;

/// The calculations an app has, added with
/// [`add_calculation`](EffectApp::add_calculation).
pub type CalculationRegistry = CodeRegistry<AnyCalculation, dyn CalculationFn>;

/// The routes an app has, with their routers, added with
/// [`add_route`](EffectApp::add_route).
///
/// Effects' data names a route by name, and executions by type.
pub type RouteRegistry = CodeRegistry<AnyRoute, dyn RouteFn>;

impl<K: Kind, F: ?Sized> CodeRegistry<K, F> {
    /// With `code`: for a registry built by hand.
    #[must_use]
    pub fn with<C: CodeFor<F> + Keyed<Kind = K>>(mut self, code: C) -> Self {
        self.add(code);
        self
    }

    /// Adds `code`, or replaces what its type added: its number.
    pub fn add<C: CodeFor<F> + Keyed<Kind = K>>(&mut self, code: C) -> Id<K> {
        self.0.add::<C>(C::KEY, code.code())
    }
}

impl<K: Kind, F: ?Sized> Default for CodeRegistry<K, F> {
    fn default() -> Self {
        Self(Registry::default())
    }
}

impl<K: Kind, F: ?Sized> Clone for CodeRegistry<K, F> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<K: Kind, F: ?Sized> std::fmt::Debug for CodeRegistry<K, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CodeRegistry").field(&self.0).finish()
    }
}

/// A value that's code a [`CodeRegistry`] of `F`s holds, as it's added.
///
/// An [`Execution`] that's an [`ExecutionFn`], a [`Calculation`] that's a
/// [`CalculationFn`], a [`Route`] that's a [`RouteFn`]: implemented for each.
pub trait CodeFor<F: ?Sized>: Keyed + Sized + 'static {
    /// It, as the registry holds it, with what it captures.
    fn code(self) -> Code<F>;
}

impl<E: Execution + ExecutionFn> CodeFor<dyn ExecutionFn> for E {
    fn code(self) -> Code<dyn ExecutionFn> {
        Code::new(Arc::new(self), E::captures(), E::routes())
    }
}

impl<C: Calculation + CalculationFn> CodeFor<dyn CalculationFn> for C {
    fn code(self) -> Code<dyn CalculationFn> {
        Code::new(Arc::new(self), C::captures(), Vec::new())
    }
}

impl<R: Route + RouteFn> CodeFor<dyn RouteFn> for R {
    fn code(self) -> Code<dyn RouteFn> {
        Code::new(Arc::new(self), R::captures(), Vec::new())
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

    /// Adds `route`: all an effect routes into it, routed into changes to
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
