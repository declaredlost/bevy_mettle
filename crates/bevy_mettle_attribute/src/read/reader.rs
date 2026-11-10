//! Reading attributes in context, in systems and on the world.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_mettle_core::registry::AttributeId;
use bevy_mettle_core::tag::OwnedTags;

use crate::Attribute;
use crate::modifier::Operation;
use crate::read::{AttributeContext, Breakdown};
use crate::registry::{AttributeRegistry, AttributeState};

/// A system parameter: actors' attributes, read in a context such as a hit's.
///
/// Its tags, its target's, the facts it supplies, its source. With nothing in
/// play, read [`Current`](crate::Current) instead.
///
/// ```ignore
/// fn strike(attributes: AttributeReader) {
///     let context = AttributeContext::new().tag("delivery.attack").fact("fact.flown", 7.0);
///     let damage = attributes.value::<Damage>(caster, &context);
///     let fire = AttributeContext::new().tag("damage.elemental.fire");
///     let resistance = attributes.value::<Resistance>(target, &fire);
/// }
/// ```
#[derive(SystemParam)]
pub struct AttributeReader<'w, 's> {
    registry: Res<'w, AttributeRegistry>,
    actors: Query<'w, 's, (&'static AttributeState, &'static OwnedTags)>,
}

impl AttributeReader<'_, '_> {
    /// `actor`'s `A` in `context`, if it has `A`.
    #[must_use]
    pub fn value<A: Attribute>(&self, actor: Entity, context: &AttributeContext) -> Option<f32> {
        self.value_at(actor, self.registry.id_of::<A>()?, context)
    }

    /// `actor`'s `attribute` in `context`, if it has it.
    #[must_use]
    pub fn value_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<f32> {
        let (state, owned) = self.actors.get(actor).ok()?;
        state
            .has(attribute)
            .then(|| state.value(&self.registry, owned, attribute, context))
    }

    /// `actor`'s base of `attribute`, if it has it.
    #[must_use]
    pub fn base_at(&self, actor: Entity, attribute: AttributeId) -> Option<f32> {
        let (state, _) = self.actors.get(actor).ok()?;
        state.cached_base(attribute)
    }

    /// `actor`'s `A`'s modifiers in `context`, summed but not applied: for an
    /// attacker's and a target's to be combined, or a tooltip.
    #[must_use]
    pub fn breakdown<A: Attribute>(
        &self,
        actor: Entity,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        self.breakdown_at(actor, self.registry.id_of::<A>()?, context)
    }

    /// `actor`'s `attribute`'s modifiers in `context`, summed but not applied.
    #[must_use]
    pub fn breakdown_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        let (state, owned) = self.actors.get(actor).ok()?;
        state
            .has(attribute)
            .then(|| state.breakdown(&self.registry, owned, attribute, context))
    }

    /// `actor`'s owned tags, if it has attributes.
    #[must_use]
    pub fn owned(&self, actor: Entity) -> Option<&OwnedTags> {
        self.actors.get(actor).ok().map(|(_, owned)| owned)
    }
}

/// Reading and changing attributes on the world, without their types.
///
/// For exclusive systems, effects and tests.
pub trait AttributeWorld {
    /// `A`'s number in this world's app, if it's added: as
    /// `world.component_id::<T>()` is to a component.
    fn attribute_id<A: Attribute>(&self) -> Option<AttributeId>;

    /// `actor`'s `attribute` in `context`, if it has it.
    fn value_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<f32>;

    /// `actor`'s `attribute`'s modifiers in `context`, summed but not applied.
    fn breakdown_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<Breakdown>;

    /// Changes `actor`'s `attribute`'s base by `op` and `value`, as an instant
    /// effect does (damage taken off life), rounded and kept within its limits:
    /// as [`Operation::apply`] says. Whether it has `attribute`.
    ///
    /// The limits are read as the bases are now, though they changed since
    /// attributes were last kept.
    fn apply_to_base(
        &mut self,
        actor: Entity,
        attribute: AttributeId,
        op: Operation,
        value: f32,
    ) -> bool;

    /// What `actor`'s `attribute`'s base would be were `base` changed by `op`
    /// and `value`, as [`apply_to_base`](Self::apply_to_base) changes it:
    /// rounded and kept within its limits, read as the bases are now. `None`
    /// if there's no such attribute.
    fn base_after(
        &mut self,
        actor: Entity,
        attribute: AttributeId,
        base: f32,
        op: Operation,
        value: f32,
    ) -> Option<f32>;

    /// `actor`'s `attribute`'s base, if it has it.
    fn base_at(&self, actor: Entity, attribute: AttributeId) -> Option<f32>;
}

impl AttributeWorld for World {
    fn attribute_id<A: Attribute>(&self) -> Option<AttributeId> {
        self.get_resource::<AttributeRegistry>()?.id_of::<A>()
    }

    fn value_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<f32> {
        let registry = self.get_resource::<AttributeRegistry>()?;
        let state = self.get::<AttributeState>(actor)?;
        let owned = self.get::<OwnedTags>(actor)?;
        state
            .has(attribute)
            .then(|| state.value(registry, owned, attribute, context))
    }

    fn breakdown_at(
        &self,
        actor: Entity,
        attribute: AttributeId,
        context: &AttributeContext,
    ) -> Option<Breakdown> {
        let registry = self.get_resource::<AttributeRegistry>()?;
        let state = self.get::<AttributeState>(actor)?;
        let owned = self.get::<OwnedTags>(actor)?;
        state
            .has(attribute)
            .then(|| state.breakdown(registry, owned, attribute, context))
    }

    fn base_at(&self, actor: Entity, attribute: AttributeId) -> Option<f32> {
        let registry = self.get_resource::<AttributeRegistry>()?;
        (registry.get(attribute)?.get_base)(self, actor)
    }

    fn apply_to_base(
        &mut self,
        actor: Entity,
        attribute: AttributeId,
        op: Operation,
        value: f32,
    ) -> bool {
        let Some(registry) = self.get_resource::<AttributeRegistry>().cloned() else {
            return false;
        };
        let Some(info) = registry.get(attribute) else {
            return false;
        };
        let Some(base) = (info.get_base)(self, actor) else {
            return false;
        };
        let Some(kept) = self.base_after(actor, attribute, base, op, value) else {
            return false;
        };
        (info.set_base)(self, actor, kept);
        if let Some(mut state) = self.get_mut::<AttributeState>(actor) {
            state.set_base(attribute, kept);
        }
        true
    }

    fn base_after(
        &mut self,
        actor: Entity,
        attribute: AttributeId,
        base: f32,
        op: Operation,
        value: f32,
    ) -> Option<f32> {
        let registry = self.get_resource::<AttributeRegistry>()?.clone();
        registry.get(attribute)?;
        sync_bases(self, &registry, actor, attribute);
        let changed = op.apply(base, value);
        Some(
            match (
                self.get::<AttributeState>(actor),
                self.get::<OwnedTags>(actor),
            ) {
                (Some(state), Some(owned)) => {
                    state.within_limits(&registry, owned.counts(), attribute, changed)
                }
                _ => changed,
            },
        )
    }
}

/// Syncs `actor`'s cached bases of what keeping `attribute` within its
/// limits reads from its `Base`s as they are now: they may have changed since
/// attributes were last kept.
fn sync_bases(
    world: &mut World,
    registry: &AttributeRegistry,
    actor: Entity,
    attribute: AttributeId,
) {
    let Some(state) = world.get::<AttributeState>(actor) else {
        return;
    };
    let changed: Vec<(AttributeId, f32)> = state
        .limits_read(registry, attribute)
        .into_iter()
        .filter_map(|id| {
            let base = (registry.get(id)?.get_base)(world, actor)?;
            (state.cached_base(id) != Some(base)).then_some((id, base))
        })
        .collect();
    if changed.is_empty() {
        return;
    }
    if let Some(mut state) = world.get_mut::<AttributeState>(actor) {
        for (attribute, base) in changed {
            state.set_base(attribute, base);
        }
    }
}
