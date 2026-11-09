//! An effect's context as it's replicated: what the game added, serialized.

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::data::first_warning;
use super::{AbilityRef, ContextDataId, ContextDataRegistry, EffectContext, Hit};

/// An [`EffectContext`] as it's replicated: what the game added, serialized.
///
/// What an effect's replica and a cue's burst carry. Its entities are mapped
/// as it arrives, those in what the game added too.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContextReplica {
    /// Who's responsible.
    pub source: Option<Entity>,
    /// What struck.
    pub causer: Option<Entity>,
    /// What it came from.
    pub object: Option<Entity>,
    /// The ability that applied it.
    pub ability: Option<AbilityRef>,
    /// Where it hit.
    pub hit: Option<Hit>,
    /// Where it came from.
    pub origin: Option<Vec3>,
    /// What the game added, those it added to the app.
    pub data: Vec<DataReplica>,
}

/// What a game added to a context, as it's replicated, serialized.
///
/// Its type's number, its bytes, and its entities beside them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataReplica {
    /// Its type.
    pub id: ContextDataId,
    /// Its entities, mapped as it arrives.
    pub entities: Vec<Entity>,
    /// It, serialized, its entities taken out.
    pub bytes: Vec<u8>,
}

impl ContextReplica {
    /// `context` as it's replicated: what the game added, those `registry`
    /// has, serialized.
    #[must_use]
    pub fn of(context: &EffectContext, registry: &ContextDataRegistry) -> Self {
        let data = context
            .data
            .iter()
            .filter_map(|data| {
                let type_id = data.as_any().type_id();
                let Some(id) = registry.id_of_type(type_id) else {
                    if first_warning(format!("unadded {}", data.name())) {
                        warn!(
                            "context data `{}` isn't added (`add_context_data`): not replicated",
                            data.name()
                        );
                    }
                    return None;
                };
                match data.encode() {
                    Ok((entities, bytes)) => Some(DataReplica {
                        id,
                        entities,
                        bytes,
                    }),
                    Err(error) => {
                        if first_warning(format!("unserialized {}", data.name())) {
                            warn!(
                                "context data `{}` didn't serialize ({error}): not replicated",
                                data.name()
                            );
                        }
                        None
                    }
                }
            })
            .collect();
        Self {
            source: context.source,
            causer: context.causer,
            object: context.object,
            ability: context.ability,
            hit: context.hit,
            origin: context.origin,
            data,
        }
    }

    /// The context it replicates: what the game added, those `registry` has,
    /// deserialized.
    #[must_use]
    pub fn context(&self, registry: &ContextDataRegistry) -> EffectContext {
        let mut context = EffectContext {
            source: self.source,
            causer: self.causer,
            object: self.object,
            ability: self.ability,
            hit: self.hit,
            origin: self.origin,
            data: Vec::new(),
        };
        for data in &self.data {
            if let Some(decoded) = registry.decode(data.id, &data.bytes, &data.entities) {
                context.insert(decoded);
            } else if first_warning(format!("undecoded {:?}", data.id)) {
                warn!(
                    "context data {:?} isn't added here, or its bytes aren't of it: dropped",
                    data.id
                );
            }
        }
        context
    }
}

impl MapEntities for ContextReplica {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        for entity in [&mut self.source, &mut self.causer, &mut self.object]
            .into_iter()
            .flatten()
        {
            *entity = mapper.get_mapped(*entity);
        }
        for data in &mut self.data {
            for entity in &mut data.entities {
                *entity = mapper.get_mapped(*entity);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::entity::{EntityHashMap, EntityMapper, MapEntities};
    use bevy::prelude::*;
    use bevy_mettle_core::registry::AbilityId;
    use serde::{Deserialize, Serialize};

    use super::{ContextReplica, DataReplica};
    use crate::context::{AbilityRef, ContextData, ContextDataRegistry, EffectContext, Hit};

    /// A shot: whether it was a headshot, and the entities it names.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, MapEntities)]
    struct Shot {
        headshot: bool,
        #[entities]
        marked: Entity,
        #[entities]
        by: Option<Entity>,
        #[entities]
        through: Vec<Entity>,
    }

    impl ContextData for Shot {
        const NAME: &'static str = "shot";
    }

    /// The wind: no entities.
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize, MapEntities)]
    struct Wind {
        speed: f32,
    }

    impl ContextData for Wind {
        const NAME: &'static str = "wind";
    }

    /// Maps each entity as it says, and any other to itself.
    struct Moved(EntityHashMap<Entity>);

    impl EntityMapper for Moved {
        fn get_mapped(&mut self, source: Entity) -> Entity {
            self.0.get(&source).copied().unwrap_or(source)
        }

        fn set_mapped(&mut self, _source: Entity, _target: Entity) {}
    }

    fn entity(index: u32) -> Entity {
        Entity::from_raw_u32(index).unwrap()
    }

    /// A registry with the shot, then the wind.
    fn registry() -> ContextDataRegistry {
        let mut registry = ContextDataRegistry::default();
        registry.add::<Shot>();
        registry.add::<Wind>();
        registry
    }

    fn shot() -> Shot {
        Shot {
            headshot: true,
            marked: entity(5),
            by: Some(entity(6)),
            // The marked one twice: each is mapped where it is.
            through: vec![entity(7), entity(5)],
        }
    }

    /// A context with everything on it.
    fn context() -> EffectContext {
        EffectContext::default()
            .source(entity(1))
            .causer(entity(2))
            .object(entity(3))
            .ability(AbilityRef {
                id: AbilityId::new(4),
                level: 2.0,
            })
            .hit(Hit {
                point: Vec3::new(1.0, 2.0, 3.0),
                normal: Vec3::Y,
            })
            .origin(Vec3::new(-4.0, 0.0, 9.0))
            .with(shot())
            .with(Wind { speed: 3.5 })
    }

    #[test]
    fn a_context_comes_back_as_it_was() {
        let registry = registry();
        let context = context();
        let replica = ContextReplica::of(&context, &registry);
        assert_eq!(replica.data.len(), 2, "both types, serialized");
        assert_eq!(replica.context(&registry), context);
        let empty = EffectContext::default();
        assert_eq!(
            ContextReplica::of(&empty, &registry).context(&registry),
            empty
        );
    }

    #[test]
    fn a_replica_survives_being_sent() {
        let registry = registry();
        let replica = ContextReplica::of(&context(), &registry);
        let bytes = postcard::to_allocvec(&replica).unwrap();
        let sent: ContextReplica = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(sent, replica);
        assert_eq!(sent.context(&registry), context());
    }

    #[test]
    fn entities_in_what_the_game_added_are_mapped() {
        let registry = registry();
        let mut replica = ContextReplica::of(&context(), &registry);
        let mut moved = Moved(
            (1..=7)
                .map(|index| (entity(index), entity(index + 10)))
                .collect(),
        );
        replica.map_entities(&mut moved);
        let mapped = replica.context(&registry);
        assert_eq!(mapped.source, Some(entity(11)));
        assert_eq!(mapped.causer, Some(entity(12)));
        assert_eq!(mapped.object, Some(entity(13)));
        assert_eq!(
            mapped.get::<Shot>(),
            Some(&Shot {
                headshot: true,
                marked: entity(15),
                by: Some(entity(16)),
                through: vec![entity(17), entity(15)],
            })
        );
        assert_eq!(mapped.get::<Wind>(), Some(&Wind { speed: 3.5 }));
        assert_eq!(mapped.ability, context().ability, "not an entity");
        assert_eq!(mapped.hit, context().hit);
        assert_eq!(mapped.origin, context().origin);
    }

    #[test]
    fn a_type_not_added_is_dropped() {
        // The wind isn't added: not sent.
        let mut shots = ContextDataRegistry::default();
        shots.add::<Shot>();
        let replica = ContextReplica::of(&context(), &shots);
        assert_eq!(replica.data.len(), 1);
        let decoded = replica.context(&shots);
        assert_eq!(decoded.get::<Shot>(), Some(&shot()));
        assert_eq!(decoded.get::<Wind>(), None);

        // Sent, but not added where it arrives: dropped there.
        let replica = ContextReplica::of(&context(), &registry());
        let decoded = replica.context(&shots);
        assert_eq!(decoded.get::<Shot>(), Some(&shot()));
        assert_eq!(decoded.get::<Wind>(), None);
        assert_eq!(decoded.source, Some(entity(1)), "the rest kept");
    }

    #[test]
    fn bytes_of_the_wrong_shape_are_dropped() {
        let registry = registry();
        let shot_id = registry.id_of::<Shot>().unwrap();
        let wind_id = registry.id_of::<Wind>().unwrap();
        let mut replica = ContextReplica::of(&context(), &registry);
        let whole = replica.data[0].bytes.clone();
        let mut trailing = whole.clone();
        trailing.push(0);
        for bytes in [
            Vec::new(),
            vec![0xFF],
            whole[..whole.len() - 1].to_vec(),
            trailing,
        ] {
            replica.data = vec![
                DataReplica {
                    id: shot_id,
                    entities: Vec::new(),
                    bytes,
                },
                DataReplica {
                    id: wind_id,
                    entities: Vec::new(),
                    bytes: postcard::to_allocvec(&Wind { speed: 1.0 }).unwrap(),
                },
            ];
            let decoded = replica.context(&registry);
            assert_eq!(decoded.get::<Shot>(), None, "dropped");
            assert_eq!(
                decoded.get::<Wind>(),
                Some(&Wind { speed: 1.0 }),
                "the rest kept"
            );
            assert_eq!(decoded.source, Some(entity(1)));
        }
    }
}
