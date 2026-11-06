//! Sources of modifiers, and the characters they count for.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::modifier::Modifier;

/// Modifiers, on whatever holds them: an item, an effect, or the actor itself.
///
/// On a source, they count for the actor it's [`ModifierOf`]; on an actor, for
/// itself.
#[derive(Component, Debug, Clone, Default, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct Modifiers(pub Vec<Modifier>);

impl FromIterator<Modifier> for Modifiers {
    fn from_iter<I: IntoIterator<Item = Modifier>>(modifiers: I) -> Self {
        Self(modifiers.into_iter().collect())
    }
}

/// On a source of modifiers: the actor its [`Modifiers`] count for.
///
/// Equipping an item is adding this; taking it off, or despawning the source,
/// takes its modifiers away with it.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
#[relationship(relationship_target = ModifierSources)]
pub struct ModifierOf(pub Entity);

impl MapEntities for ModifierOf {
    /// Maps the actor, as a replicated source arrives from another world.
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        self.0 = mapper.get_mapped(self.0);
    }
}

/// On an actor: the sources whose modifiers count for it.
///
/// Kept by [`ModifierOf`]; not for setting by hand.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = ModifierOf)]
pub struct ModifierSources(Vec<Entity>);

impl ModifierSources {
    /// The sources, in the order they were added.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}
