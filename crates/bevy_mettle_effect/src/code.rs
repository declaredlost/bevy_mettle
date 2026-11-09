//! What effects run in code: executions, calculations, and meta attributes
//! and their routers.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_attribute::Attribute;
use bevy_mettle_attribute::modifier::Operation;
use bevy_mettle_attribute::read::{AttributeContext, AttributeWorld, Breakdown};
use bevy_mettle_attribute::registry::AnyAttribute;
use bevy_mettle_core::tag::{OwnedTags, Tag};
use bevy_mettle_core::{Id, IdTarget};

use crate::EffectSpec;

/// What an execution or a calculation sees: the world, the target, the spec.
///
/// The world read only, and how many stacks there are.
pub struct EffectContext<'a> {
    /// The world.
    pub world: &'a World,
    /// Who it's applied to.
    pub target: Entity,
    /// What's applied.
    pub spec: &'a EffectSpec,
    /// How many stacks of it there are: 1 unless it stacks.
    pub stacks: u32,
}

impl EffectContext<'_> {
    /// The spec's level.
    #[must_use]
    pub fn level(&self) -> f32 {
        self.spec.level
    }

    /// What `param` was set to, if it was.
    #[must_use]
    pub fn param(&self, param: &Id<Tag>) -> Option<f32> {
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

    /// `entity`'s `A` in `context`, if it has `A`.
    #[must_use]
    pub fn value<A: Attribute>(&self, entity: Entity, context: &AttributeContext) -> Option<f32> {
        self.world.value_at(entity, &A::ID, context)
    }

    /// `entity`'s `attribute` in `context`, if it has it.
    #[must_use]
    pub fn value_at(
        &self,
        entity: Entity,
        attribute: &Id<AnyAttribute>,
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
        self.world.breakdown_at(entity, &A::ID, context)
    }

    /// `entity`'s `attribute`'s modifiers in `context`, summed but not applied,
    /// if it has it.
    #[must_use]
    pub fn breakdown_at(
        &self,
        entity: Entity,
        attribute: &Id<AnyAttribute>,
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

/// A change an execution or a router makes to a base, as an instant modifier:
/// its target's, unless it says.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseChange {
    /// The attribute, or a meta attribute to route.
    pub attribute: Id<AnyAttribute>,
    /// How.
    pub op: Operation,
    /// By how much.
    pub value: f32,
    /// Whose base.
    pub on: Whose,
}

impl BaseChange {
    /// `op` of `value` to `A`'s base.
    #[must_use]
    pub fn new<A: Attribute>(op: Operation, value: f32) -> Self {
        Self {
            attribute: A::ID,
            op,
            value,
            on: Whose::Target,
        }
    }

    /// `value` to the meta attribute `M`, for its router: what an execution
    /// deals, before shields and resistances.
    #[must_use]
    pub fn meta<M: MetaAttribute>(value: f32) -> Self {
        Self {
            attribute: M::ID,
            op: Operation::Flat,
            value,
            on: Whose::Target,
        }
    }

    /// To the source's base, not the target's.
    #[must_use]
    pub fn on_source(mut self) -> Self {
        self.on = Whose::Source;
        self
    }
}

/// A meta attribute: a number an effect deals to its target that no actor
/// has, routed into attributes they do as it executes.
///
/// Incoming damage, say: dealt by every hit, then routed through armour and
/// shields into life by one router, not each hit's own code. Derive it, and
/// add it with its router:
///
/// ```ignore
/// #[derive(MetaAttribute)]
/// pub struct IncomingDamage;
///
/// app.add_meta_attribute::<IncomingDamage>(route_damage);
/// ```
///
/// Data names it as it does attributes, `"incoming_damage"`, in the flat
/// modifiers of an effect that executes: instant, or periodic. It has no base,
/// limits or rounding, isn't held by actors, and isn't replicated: what it's
/// routed into is.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't a meta attribute",
    label = "not a meta attribute",
    note = "derive it: `#[derive(MetaAttribute)]`"
)]
pub trait MetaAttribute: Send + Sync + 'static {
    /// The name data names it by: `"incoming_damage"`.
    const NAME: &'static str;

    /// Its id: its name, among the attributes'.
    const ID: Id<AnyAttribute> = Id::from_static(Self::NAME);
}

pub use bevy_mettle_macros::MetaAttribute;

/// An execution: what it changes, worked out in code.
pub type ExecutionFn = Arc<dyn Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync>;

/// A calculation: a magnitude, worked out in code.
pub type CalculationFn = Arc<dyn Fn(&EffectContext) -> f32 + Send + Sync>;

/// A router: what a meta attribute's amount, all an execution dealt of it,
/// changes in attributes actors have.
pub type RouterFn = Arc<dyn Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync>;

/// What effects run in code, by name: their executions, calculations, and
/// meta attributes with their routers.
///
/// What effects' data names, and their registry is checked against. A
/// resource, added to with [`add_execution`](EffectCodeApp::add_execution),
/// [`add_calculation`](EffectCodeApp::add_calculation) and
/// [`add_meta_attribute`](EffectCodeApp::add_meta_attribute).
#[derive(Resource, Clone, Default)]
pub struct EffectCode {
    pub(crate) executions: HashMap<Id<Execution>, ExecutionFn>,
    pub(crate) calculations: HashMap<Id<Calculation>, CalculationFn>,
    pub(crate) routers: HashMap<Id<AnyAttribute>, RouterFn>,
}

impl EffectCode {
    /// Nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The execution `name` is `execution`.
    #[must_use]
    pub fn execution(
        mut self,
        name: impl Into<Id<Execution>>,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> Self {
        self.executions.insert(name.into(), Arc::new(execution));
        self
    }

    /// The calculation `name` is `calculation`.
    #[must_use]
    pub fn calculation(
        mut self,
        name: impl Into<Id<Calculation>>,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> Self {
        self.calculations.insert(name.into(), Arc::new(calculation));
        self
    }

    /// The meta attribute `M`, routed by `router`.
    #[must_use]
    pub fn meta_attribute<M: MetaAttribute>(
        mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> Self {
        self.routers.insert(M::ID, Arc::new(router));
        self
    }
}

/// An execution: a formula in code run as an effect executes, such as damage.
///
/// Run as an instant or periodic effect executes; named by an `Id<Execution>`.
#[derive(IdTarget)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Execution {}

/// A calculation: a magnitude worked out in code, named by an
/// `Id<Calculation>`.
#[derive(IdTarget)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Calculation {}

/// Adding effect code to an app: executions and calculations by name.
pub trait EffectCodeApp {
    /// Adds the execution `name`: a formula run as an effect naming it
    /// executes, such as damage.
    fn add_execution(
        &mut self,
        name: impl Into<Id<Execution>>,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self;

    /// Adds the calculation `name`: a magnitude an effect naming it works out
    /// in code.
    fn add_calculation(
        &mut self,
        name: impl Into<Id<Calculation>>,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> &mut Self;

    /// Adds the meta attribute `M`, routed by `router`: all an execution
    /// dealt of it, into changes to attributes actors have.
    fn add_meta_attribute<M: MetaAttribute>(
        &mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self;
}

impl EffectCodeApp for App {
    fn add_execution(
        &mut self,
        name: impl Into<Id<Execution>>,
        execution: impl Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self {
        let mut code = self.world_mut().get_resource_or_init::<EffectCode>();
        code.executions.insert(name.into(), Arc::new(execution));
        self
    }

    fn add_calculation(
        &mut self,
        name: impl Into<Id<Calculation>>,
        calculation: impl Fn(&EffectContext) -> f32 + Send + Sync + 'static,
    ) -> &mut Self {
        let mut code = self.world_mut().get_resource_or_init::<EffectCode>();
        code.calculations.insert(name.into(), Arc::new(calculation));
        self
    }

    fn add_meta_attribute<M: MetaAttribute>(
        &mut self,
        router: impl Fn(&EffectContext, f32) -> Vec<BaseChange> + Send + Sync + 'static,
    ) -> &mut Self {
        let mut code = self.world_mut().get_resource_or_init::<EffectCode>();
        code.routers.insert(M::ID, Arc::new(router));
        self
    }
}
