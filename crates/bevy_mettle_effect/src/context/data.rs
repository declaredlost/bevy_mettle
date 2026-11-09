//! What a game adds to an effect's context: its own types, numbered as added.

use std::any::Any;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy_mettle_core::registry::{Key, Kind, Registry};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// What a game adds to an effect's context: its own type, as a shot's.
///
/// Derive it, add it, and put it on a spec:
///
/// ```ignore
/// #[derive(ContextData, Clone, Debug, PartialEq, Serialize, Deserialize, MapEntities)]
/// struct Shot { cartridge: u32, headshot: bool }
///
/// app.add_context_data::<Shot>();
/// commands.entity(target).apply_effect(Damage.spec().causer(arrow).with(Shot { cartridge: 7, headshot: true }));
/// // In an execution: `cx.get::<Shot>()`.
/// ```
///
/// Unreal's context subclassed. Replicated with what carries the context
/// (an effect's replica, a cue's burst): serialized, its entities mapped as
/// it arrives. Added in the same order on a server and its clients, so each
/// numbers it the same.
///
/// Serialized compactly, as lightyear sends messages: not self-describing, so
/// serde's `skip_serializing_if`, `flatten` and `untagged` don't round-trip.
/// What one sends that doesn't is dropped with a warning.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't context data",
    label = "not context data",
    note = "derive it: `#[derive(ContextData)]`, with `Clone`, `Debug`, `PartialEq`, `Serialize`, `Deserialize` and `MapEntities`"
)]
pub trait ContextData:
    Clone + Debug + PartialEq + Serialize + DeserializeOwned + MapEntities + Send + Sync + 'static
{
    /// The name it's added by: `"shot"`.
    const NAME: &'static str;
}

pub use bevy_mettle_macros::ContextData;

/// The context data kind: what a [`ContextDataId`] numbers.
#[derive(Kind)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AnyContextData {}

/// A context data type's number in an app, given as it's added to it.
///
/// What it's replicated by.
pub type ContextDataId = Key<AnyContextData>;

/// What a game added to a context, its type erased.
pub(super) type Data = Arc<dyn Erased>;

/// A [`ContextData`], its type erased.
pub(super) trait Erased: Debug + Send + Sync {
    /// It, to downcast.
    fn as_any(&self) -> &dyn Any;

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

    fn name(&self) -> &'static str {
        C::NAME
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

/// What a context data type is decoded by, from its bytes and entities.
#[derive(Clone, Copy)]
pub struct ContextDataFns {
    decode: fn(&[u8], &[Entity]) -> Option<Data>,
}

impl Debug for ContextDataFns {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextDataFns").finish_non_exhaustive()
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

/// The types a game adds to effects' contexts: what they're replicated by.
///
/// Each numbered as it's added, as Bevy numbers components: added in the same
/// order on a server and its clients, each numbers them the same. Added to
/// with [`add_context_data`](crate::code::EffectApp::add_context_data). Read
/// as a [`Registry`]: `id(name)`, `id_of::<C>()`, `name(id)`.
#[derive(Resource, Deref, Clone, Default, Debug)]
pub struct ContextDataRegistry(Registry<AnyContextData, ContextDataFns>);

impl ContextDataRegistry {
    /// Adds `C`, or replaces what's there: its number.
    pub(crate) fn add<C: ContextData>(&mut self) -> ContextDataId {
        self.0.add::<C>(
            C::NAME,
            ContextDataFns {
                decode: decode::<C>,
            },
        )
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
pub(super) fn first_warning(what: String) -> bool {
    static WARNED: Mutex<Option<HashSet<String>>> = Mutex::new(None);
    WARNED.lock().map_or(true, |mut warned| {
        warned.get_or_insert_default().insert(what)
    })
}
