//! What a game adds to an effect's context: its own types, reflected and
//! numbered by type path.

use std::any::Any;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::reflect::{CreateTypeData, TypeRegistry};
use bevy_mettle_core::registry::{Id, Kind, Registry};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// What a game adds to an effect's context: its own type, as a shot's.
///
/// Any reflected type with these derives is; `#[reflect(ContextData)]`
/// registers it as one, as Bevy registers components:
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy::ecs::entity::MapEntities;
/// # use bevy_mettle_effect::prelude::*;
/// # use serde::{Deserialize, Serialize};
/// # #[derive(Effect)] struct ChainLightning;
/// # let (mage, bolt) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
/// /// Who the lightning already struck, for the next jump to skip.
/// #[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
/// #[reflect(ContextData)]
/// struct Chain {
///     #[entities]
///     struck: Vec<Entity>,
/// }
///
/// App::new().register_type::<Chain>();
/// let spec = ChainLightning
///     .source(mage)
///     .causer(bolt)
///     .with(Chain { struck: vec![mage] });
/// // In an execution: `cx.get::<Chain>()`.
/// assert!(spec.context.get::<Chain>().is_some());
/// ```
///
/// Unreal's context subclassed, for **structured** data: a flag (a critical,
/// a headshot) is a tag the application carries, which conditions, immunity
/// and cues see; a number is a [`param`](crate::EffectSpec::param); what
/// struck and what it came from are the context's `causer` and `object`.
///
/// Replicated with what carries the context (an effect's replica, a cue's
/// burst): serialized, its entities mapped as it arrives. Numbered by its type
/// path, among the types registered as context data once the app's built, so
/// a server and its clients register them in whatever order.
///
/// Serialized compactly, as lightyear sends messages: not self-describing, so
/// serde's `skip_serializing_if`, `flatten` and `untagged` don't round-trip.
/// What one sends that doesn't is dropped with a warning.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't context data",
    label = "not context data",
    note = "derive `Reflect`, `Clone`, `PartialEq`, `Serialize`, `Deserialize` and `MapEntities`"
)]
pub trait ContextData:
    Reflect
    + TypePath
    + Clone
    + PartialEq
    + Serialize
    + DeserializeOwned
    + MapEntities
    + Send
    + Sync
    + 'static
{
}

impl<C> ContextData for C where
    C: Reflect
        + TypePath
        + Clone
        + PartialEq
        + Serialize
        + DeserializeOwned
        + MapEntities
        + Send
        + Sync
        + 'static
{
}

/// The context data kind: what a [`ContextDataId`] numbers, by type path.
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyContextData {}

/// A context data type's number in an app, in order of name.
///
/// What it's replicated by.
pub type ContextDataId = Id<AnyContextData>;

/// What a game added to a context, its type erased.
pub(super) type Data = Arc<dyn Erased>;

/// A [`ContextData`], its type erased.
pub(super) trait Erased: Send + Sync {
    /// It, to downcast.
    fn as_any(&self) -> &dyn Any;

    /// It, reflected: as it's shown.
    fn as_reflect(&self) -> &dyn Reflect;

    /// Its type's name, as it's added.
    fn name(&self) -> &'static str;

    /// Whether it's `other`.
    fn same(&self, other: &dyn Erased) -> bool;

    /// It serialized, its entities in a list beside it: the list mapped as
    /// it's replicated, put back as it's decoded.
    fn encode(&self) -> Result<(Vec<Entity>, Vec<u8>), postcard::Error>;
}

impl<C: ContextData> Erased for C {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_reflect(&self) -> &dyn Reflect {
        self
    }

    fn name(&self) -> &'static str {
        C::type_path()
    }

    fn same(&self, other: &dyn Erased) -> bool {
        other.as_any().downcast_ref::<C>() == Some(self)
    }

    fn encode(&self) -> Result<(Vec<Entity>, Vec<u8>), postcard::Error> {
        let mut data = self.clone();
        let mut taken = Take(Vec::new());
        data.map_entities(&mut taken);
        Ok((taken.0, postcard::to_allocvec(&data)?))
    }
}

impl Debug for dyn Erased {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(self.as_reflect(), f)
    }
}

impl PartialEq for dyn Erased {
    fn eq(&self, other: &Self) -> bool {
        self.same(other)
    }
}

/// Takes each entity out of what's encoded, into a list: what's left in its
/// place is its index in it.
struct Take(Vec<Entity>);

impl EntityMapper for Take {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        let index = u32::try_from(self.0.len()).unwrap_or(u32::MAX - 1);
        self.0.push(source);
        Entity::from_raw_u32(index).unwrap_or(Entity::PLACEHOLDER)
    }

    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

/// Puts each entity back into what's decoded, from the list beside it.
struct Put<'a>(&'a [Entity]);

impl EntityMapper for Put<'_> {
    fn get_mapped(&mut self, source: Entity) -> Entity {
        self.0
            .get(source.index_u32() as usize)
            .copied()
            .unwrap_or(Entity::PLACEHOLDER)
    }

    fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
}

/// A type's registration as [`ContextData`]: `#[reflect(ContextData)]`.
///
/// What it's decoded by, from its bytes and entities, as reflection knows
/// it: Bevy's type data, as `ReflectComponent` is a component's.
#[derive(Clone, Copy)]
pub struct ReflectContextData {
    decode: fn(&[u8], &[Entity]) -> Option<Data>,
}

impl Debug for ReflectContextData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReflectContextData").finish_non_exhaustive()
    }
}

impl<C: ContextData> CreateTypeData<C> for ReflectContextData {
    fn create_type_data(_input: ()) -> Self {
        Self {
            decode: decode::<C>,
        }
    }
}

/// Decodes a `C` from `bytes`, its entities put back from `entities`.
fn decode<C: ContextData>(bytes: &[u8], entities: &[Entity]) -> Option<Data> {
    let (mut data, rest): (C, _) = postcard::take_from_bytes(bytes).ok()?;
    if !rest.is_empty() {
        return None;
    }
    data.map_entities(&mut Put(entities));
    Some(Arc::new(data))
}

/// The types registered as context data: what they're replicated by.
///
/// Every type the app's type registry has as [`ReflectContextData`], once
/// it's built, numbered in order of type path: a server and its clients
/// registering the same types number them the same, in whatever order they
/// register them. Read as a [`Registry`]: `id(name)`, `id_of::<C>()`,
/// `name(id)`.
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct ContextDataRegistry(Registry<AnyContextData, ReflectContextData>);

impl ContextDataRegistry {
    /// The types `types` has as context data, numbered in order of type path.
    #[must_use]
    pub fn of(types: &TypeRegistry) -> Self {
        let mut registry = Registry::default();
        for registration in types.iter() {
            if let Some(data) = registration.data::<ReflectContextData>() {
                let info = registration.type_info();
                registry.add_type(info.type_id(), info.type_path(), info.type_path(), *data);
            }
        }
        Self(registry.sorted())
    }

    /// Decodes what's numbered `id` from `bytes` and `entities`, if it's
    /// added and they're of it.
    pub(super) fn decode(
        &self,
        id: ContextDataId,
        bytes: &[u8],
        entities: &[Entity],
    ) -> Option<Data> {
        (self.0.get(id)?.decode)(bytes, entities)
    }
}

/// Whether `what` hasn't been warned of yet in this process: noted as it is.
///
/// What a context carries is encoded and decoded as often as effects apply
/// and cues play; a type that won't is warned of once.
pub(crate) fn first_warning(what: String) -> bool {
    static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    WARNED.lock().map_or(true, |mut warned| {
        warned.get_or_insert_default().insert(what)
    })
}
