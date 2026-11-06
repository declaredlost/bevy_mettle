//! What effects run in code: executions and calculations.

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

/// A change an execution makes to its target's base, as an instant modifier.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseChange {
    /// The attribute.
    pub attribute: Id<AnyAttribute>,
    /// How.
    pub op: Operation,
    /// By how much.
    pub value: f32,
}

impl BaseChange {
    /// `op` of `value` to `A`'s base.
    #[must_use]
    pub fn new<A: Attribute>(op: Operation, value: f32) -> Self {
        Self {
            attribute: A::ID,
            op,
            value,
        }
    }
}

/// An execution: what it changes, worked out in code.
pub type ExecutionFn = Arc<dyn Fn(&EffectContext) -> Vec<BaseChange> + Send + Sync>;

/// A calculation: a magnitude, worked out in code.
pub type CalculationFn = Arc<dyn Fn(&EffectContext) -> f32 + Send + Sync>;

/// What effects run in code, by name: their executions and calculations.
///
/// What effects' data names, and their registry is checked against.
#[derive(Clone, Default)]
pub struct EffectCode {
    pub(crate) executions: HashMap<Id<Execution>, ExecutionFn>,
    pub(crate) calculations: HashMap<Id<Calculation>, CalculationFn>,
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
