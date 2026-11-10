//! What an effect's application carries beyond its spec: who, and the rest.
//!
//! Its source, and context data: what struck ([`Causer`]), what it came from
//! ([`SourceObject`]), the ability that applied it ([`AbilityRef`]), where it
//! hit ([`Hit`]) and came from ([`Origin`]), and what the game adds of its
//! own.

mod builtin;
mod data;
mod replica;

use std::sync::Arc;

use bevy::prelude::*;

pub use self::builtin::{AbilityRef, Causer, Hit, Origin, SourceObject};
use self::data::Data;
pub(crate) use self::data::first_warning;
pub use self::data::{
    AnyContextData, ContextData, ContextDataId, ContextDataRegistry, ReflectContextData,
};
pub use self::replica::{ContextReplica, DataReplica};

/// What an effect's application carries: its source, and context data.
///
/// Unreal's `FGameplayEffectContext`. Its [`source`](Self::source) is who's
/// responsible (the archer): what the engine reads its captures and source
/// tags from. Everything else is [`ContextData`], one of each type, added
/// with [`with`](Self::with) and read with [`get`](Self::get): mettle's own,
/// [`Causer`] (the arrow), [`SourceObject`] (the bow), [`AbilityRef`],
/// [`Hit`] and [`Origin`], and the game's.
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
    /// What's added, one of each type.
    data: Vec<Data>,
}

impl EffectContext {
    /// From `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.source = Some(source);
        self
    }

    /// With `data`: in place of any of its type.
    #[must_use]
    pub fn with<C: ContextData>(mut self, data: C) -> Self {
        self.insert(Arc::new(data));
        self
    }

    /// What's added of type `C`, if anything is.
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
    /// Whether they carry the same: what's added, whatever order it was
    /// added in.
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
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
