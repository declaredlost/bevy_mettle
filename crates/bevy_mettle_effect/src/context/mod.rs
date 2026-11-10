//! What an effect's application carries beyond its spec: who, what, and where.
//!
//! Its source, what struck, what it came from, the ability that applied it,
//! where it hit and came from, and what the game adds of its own.

mod data;
mod replica;

use std::sync::Arc;

use bevy::prelude::*;
use bevy_mettle_core::registry::AbilityId;
use serde::{Deserialize, Serialize};

use self::data::Data;
pub(crate) use self::data::first_warning;
pub use self::data::{
    AnyContextData, ContextData, ContextDataId, ContextDataRegistry, ReflectContextData,
};
pub use self::replica::{ContextReplica, DataReplica};

/// What an effect's application carries: who, what struck, and where.
///
/// Unreal's `FGameplayEffectContext`. Its [`source`](Self::source) is who's
/// responsible (the archer); its [`causer`](Self::causer), what struck (the
/// arrow); its [`object`](Self::object), what it came from (the bow). The
/// game adds its own, typed, with [`with`](Self::with): a [`ContextData`].
///
/// On an [`EffectSpec`](crate::EffectSpec), set with its builders; an effect
/// it chains carries it too. Executions and calculations read it from their
/// [`ExecutionContext`](crate::code::ExecutionContext), and cues an effect
/// plays are told it.
#[derive(Debug, Clone, Default)]
pub struct EffectContext {
    /// Who's responsible: the caster, the attacker. What source tags and
    /// attribute captures are read from, and stacks are told apart by.
    pub source: Option<Entity>,
    /// What struck: the arrow, the trap, the explosion.
    pub causer: Option<Entity>,
    /// What it came from: the bow, the item, the aura.
    pub object: Option<Entity>,
    /// The ability that applied it, at the level it was activated at.
    pub ability: Option<AbilityRef>,
    /// Where it hit.
    pub hit: Option<Hit>,
    /// Where it came from: what knockback pushes away from.
    pub origin: Option<Vec3>,
    /// What the game added, one of each type.
    data: Vec<Data>,
}

impl EffectContext {
    /// From `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// Struck by `causer`.
    #[must_use]
    pub fn causer(mut self, causer: Entity) -> Self {
        self.causer = Some(causer);
        self
    }

    /// Come from `object`.
    #[must_use]
    pub fn object(mut self, object: Entity) -> Self {
        self.object = Some(object);
        self
    }

    /// Applied by `ability`.
    #[must_use]
    pub fn ability(mut self, ability: AbilityRef) -> Self {
        self.ability = Some(ability);
        self
    }

    /// Hit at `hit`.
    #[must_use]
    pub fn hit(mut self, hit: Hit) -> Self {
        self.hit = Some(hit);
        self
    }

    /// Come from `origin`.
    #[must_use]
    pub fn origin(mut self, origin: Vec3) -> Self {
        self.origin = Some(origin);
        self
    }

    /// With `data`, the game's own: in place of any of its type.
    #[must_use]
    pub fn with<C: ContextData>(mut self, data: C) -> Self {
        self.insert(Arc::new(data));
        self
    }

    /// What the game added of type `C`, if it did.
    #[must_use]
    pub fn get<C: ContextData>(&self) -> Option<&C> {
        self.data
            .iter()
            .find_map(|data| data.as_any().downcast_ref::<C>())
    }

    /// Adds `data`, in place of any of its type.
    fn insert(&mut self, data: Data) {
        let type_id = data.as_any().type_id();
        match self
            .data
            .iter_mut()
            .find(|known| known.as_any().type_id() == type_id)
        {
            Some(known) => *known = data,
            None => self.data.push(data),
        }
    }
}

impl PartialEq for EffectContext {
    /// Whether they carry the same: what the game added, whatever order it
    /// was added in.
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.causer == other.causer
            && self.object == other.object
            && self.ability == other.ability
            && self.hit == other.hit
            && self.origin == other.origin
            && self.data.len() == other.data.len()
            && self.data.iter().all(|data| {
                let type_id = data.as_any().type_id();
                other
                    .data
                    .iter()
                    .find(|known| known.as_any().type_id() == type_id)
                    .is_some_and(|known| known == data)
            })
    }
}

/// Where an effect hit: the point, and the surface's normal there.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    /// The point hit.
    pub point: Vec3,
    /// The surface's normal at it.
    pub normal: Vec3,
}

/// The ability an effect was applied by, at the level it was activated at.
///
/// Unreal's context's ability and its level. Which run it was, the effect's
/// spec says: its activation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AbilityRef {
    /// The ability.
    pub id: AbilityId,
    /// Its level as it was activated.
    pub level: f32,
}
