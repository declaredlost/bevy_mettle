//! Mettle's own context data: what struck, what from, which ability, where.

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_mettle_core::registry::AbilityId;
use serde::{Deserialize, Serialize};

use super::ReflectContextData;

/// Context data: what struck, as the arrow, the trap or the explosion did.
///
/// Its effect's source is who's responsible; this is what hit. An ability's
/// effects carry its actor's.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Reflect,
    Serialize,
    Deserialize,
    MapEntities
)]
#[reflect(ContextData)]
pub struct Causer(#[entities] pub Entity);

/// Context data: what an effect came from, as the bow, the item or the aura.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Reflect,
    Serialize,
    Deserialize,
    MapEntities
)]
#[reflect(ContextData)]
pub struct SourceObject(#[entities] pub Entity);

/// Context data: where an effect hit, the point and the surface's normal.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Reflect,
    Serialize,
    Deserialize,
    MapEntities
)]
#[reflect(ContextData)]
pub struct Hit {
    /// The point hit.
    pub point: Vec3,
    /// The surface's normal at it.
    pub normal: Vec3,
}

/// Context data: where an effect came from, what knockback pushes away from.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Reflect,
    Serialize,
    Deserialize,
    MapEntities
)]
#[reflect(ContextData)]
pub struct Origin(pub Vec3);

/// Context data: the ability that applied an effect, at its activated level.
///
/// Unreal's context's ability and its level. Which run it was, the effect's
/// spec says: its activation.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Reflect,
    Serialize,
    Deserialize,
    MapEntities
)]
#[reflect(ContextData)]
pub struct AbilityRef {
    /// The ability.
    pub id: AbilityId,
    /// Its level as it was activated.
    pub level: f32,
}
