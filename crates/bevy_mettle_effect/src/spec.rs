//! What's applied: an effect, at a level, with what code set.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy_mettle_core::Id;
use bevy_mettle_core::tag::Tag;
use serde::{Deserialize, Serialize};

use crate::data::Effect;

/// An effect about to be applied: which, at what level, with what, from whom.
///
/// Unreal's `GameplayEffectSpec`: with what the applying code set. What's
/// applied, and what's sent to replicate an active effect: its definition,
/// being data, isn't. Every field is always written, so binary formats (what
/// networking uses) read it back; data can leave out all but `effect`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectSpec {
    /// The effect.
    pub effect: Id<Effect>,
    /// Its level: what curves are read at. 1 unless it says.
    #[serde(default = "one")]
    pub level: f32,
    /// What the applying code set, for `Param` magnitudes.
    #[serde(default)]
    pub params: Vec<(Id<Tag>, f32)>,
    /// Who applied it: the caster, the attacker.
    #[serde(default)]
    pub source: Option<Entity>,
    /// The activation it was applied under, if any: what a rejected prediction
    /// takes back. One the server's own has its top bit set; a client's is
    /// its own, told apart from another client's by the source.
    #[serde(default)]
    pub activation: Option<u64>,
}

impl MapEntities for EffectSpec {
    /// Maps its source, as it arrives from another world.
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        if let Some(source) = &mut self.source {
            *source = mapper.get_mapped(*source);
        }
    }
}

fn one() -> f32 {
    1.0
}

impl EffectSpec {
    /// `effect`, at level 1, from no one.
    #[must_use]
    pub fn new(effect: impl Into<Id<Effect>>) -> Self {
        Self {
            effect: effect.into(),
            level: 1.0,
            params: Vec::new(),
            source: None,
            activation: None,
        }
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// With `param` (a tag: `param.damage`) at `value`. Setting one again
    /// replaces it.
    #[must_use]
    pub fn param(mut self, param: impl Into<Id<Tag>>, value: f32) -> Self {
        let param = param.into();
        match self.params.iter_mut().find(|(known, _)| *known == param) {
            Some((_, known)) => *known = value,
            None => self.params.push((param, value)),
        }
        self
    }

    /// From `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// Under `activation`.
    #[must_use]
    pub fn activation(mut self, activation: u64) -> Self {
        self.activation = Some(activation);
        self
    }

    /// What `param` was set to, if it was.
    #[must_use]
    pub fn param_value(&self, param: &Id<Tag>) -> Option<f32> {
        self.params
            .iter()
            .find(|(known, _)| known == param)
            .map(|(_, value)| *value)
    }

    /// The same, for `effect`: what an effect applies in a chain.
    pub(crate) fn for_effect(&self, effect: Id<Effect>) -> Self {
        Self {
            effect,
            ..self.clone()
        }
    }
}
