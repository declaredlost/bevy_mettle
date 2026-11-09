//! What a client sees of the server's abilities: replicas of them, shown as
//! granted.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::active::GrantedAbility;
use crate::{AbilityId, AbilityRegistry};

/// On a server's granted ability: what a client needs to have it too.
///
/// For the game to replicate with its `AbilityOf`.
#[derive(Component, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AbilityReplica {
    /// The ability.
    pub id: AbilityId,
    /// Its level.
    pub level: f32,
}

/// On a server: keeps each granted ability's replica.
pub(crate) fn sync_replicas(
    mut commands: Commands,
    granted: Query<(Entity, &GrantedAbility, Option<&AbilityReplica>), Changed<GrantedAbility>>,
) {
    for (entity, granted, replica) in &granted {
        let now = AbilityReplica {
            id: granted.def.id,
            level: granted.level,
        };
        if replica != Some(&now) {
            commands.entity(entity).insert(now);
        }
    }
}

/// On a client: has each ability the server's replica arrived for, and keeps
/// it as its replica says.
pub(crate) fn show_replicas(
    mut commands: Commands,
    registry: Res<AbilityRegistry>,
    arrived: Query<(Entity, &AbilityReplica), Without<GrantedAbility>>,
    mut changed: Query<(&AbilityReplica, &mut GrantedAbility), Changed<AbilityReplica>>,
) {
    for (entity, replica) in &arrived {
        let Some(def) = registry.get(replica.id).cloned() else {
            warn!("there's no ability {:?} to have", replica.id);
            continue;
        };
        commands
            .entity(entity)
            .insert(GrantedAbility::new(def, replica.level, None));
    }
    for (replica, mut granted) in &mut changed {
        if granted.def.id != replica.id {
            let Some(def) = registry.get(replica.id).cloned() else {
                warn!("there's no ability {:?} to have", replica.id);
                continue;
            };
            granted.def = def;
        }
        if granted.level.to_bits() != replica.level.to_bits() {
            granted.level = replica.level;
        }
    }
}
