//! Taking a spec's snapshot as it's made: its source's attributes, for its
//! effect's snapshot captures.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_mettle_attribute::read::{AttributeContext, AttributeReader, AttributeWorld};
use bevy_mettle_core::registry::AttributeId;
use bevy_mettle_core::tag::OwnedTags;

use crate::data::{Capture, Read};
use crate::{EffectRegistry, EffectSpec, Snapshot};

/// A system parameter: taking specs' snapshots as they're made.
///
/// Unreal's `MakeOutgoingSpec` capturing its source: what a projectile in
/// flight carries, its caster's spell power as it was cast, kept whatever
/// becomes of the caster.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// # #[derive(Effect)] #[effect(name = "fireball.hit")] struct FireballHit;
/// /// A caster casting a fireball at its target.
/// #[derive(Component)]
/// struct Casting(Entity);
///
/// /// A fireball in flight, and what it hits with.
/// #[derive(Component)]
/// struct Projectile {
///     target: Entity,
///     hit: EffectSpec,
/// }
///
/// fn fire(casting: Query<(Entity, &Casting), Added<Casting>>, snapshots: Snapshots, mut commands: Commands) {
///     for (caster, casting) in &casting {
///         let hit = snapshots.take(FireballHit.source(caster));
///         commands.spawn(Projectile { target: casting.0, hit });
///     }
/// }
/// # App::new().add_systems(Update, fire);
/// ```
///
/// An ability's behaviour takes `ActiveAbility::effect_spec`'s spec. A spec
/// applied without one takes its snapshot as it's applied, as does a capture
/// its source couldn't give yet.
#[derive(SystemParam)]
pub struct Snapshots<'w, 's> {
    effects: Res<'w, EffectRegistry>,
    attributes: AttributeReader<'w, 's>,
}

impl Snapshots<'_, '_> {
    /// `spec`, its snapshot taken from its source now.
    #[must_use]
    pub fn take(&self, spec: impl Into<EffectSpec>) -> EffectSpec {
        let spec = spec.into();
        let owned = spec
            .context
            .source
            .and_then(|source| self.attributes.owned(source));
        take(&self.effects, spec, owned, |source, attribute, hit| {
            (
                self.attributes.value_at(source, attribute, hit),
                self.attributes.base_at(source, attribute),
            )
        })
    }
}

/// `spec`, its snapshot taken from its source in `world` now.
pub(crate) fn take_in(world: &World, spec: EffectSpec) -> EffectSpec {
    let Some(effects) = world.get_resource::<EffectRegistry>() else {
        return spec;
    };
    let owned = spec
        .context
        .source
        .and_then(|source| world.get::<OwnedTags>(source));
    take(effects, spec, owned, |source, attribute, hit| {
        (
            world.value_at(source, attribute, hit),
            world.base_at(source, attribute),
        )
    })
}

/// `spec`, its snapshot taken by `read`: an attribute's current value, read
/// in a hit's context, and its base, for its effect's snapshot captures of its
/// source, which owns `owned`.
fn take(
    effects: &EffectRegistry,
    mut spec: EffectSpec,
    owned: Option<&OwnedTags>,
    read: impl Fn(Entity, AttributeId, &AttributeContext) -> (Option<f32>, Option<f32>),
) -> EffectSpec {
    let Some(source) = spec.context.source else {
        return spec;
    };
    let Some(def) = spec.effect_id(effects).and_then(|id| effects.get(id)) else {
        return spec;
    };
    for capture in &def.captures {
        if !capture.snapshot || capture.from != Capture::Source {
            continue;
        }
        // Read as a hit would be: in its effect's tags and its own, from its
        // source; its target isn't known yet.
        let mut hit = AttributeContext::new()
            .tags(&capture.read_in)
            .source(source);
        if let Some(owned) = owned {
            hit = hit.source_tags(owned);
        }
        let (current, base) = read(source, capture.attribute, &hit);
        let amount = match capture.value {
            Read::Current => current,
            Read::Base => base,
            Read::Bonus => current.zip(base).map(|(current, base)| current - base),
        };
        // What can't be read yet (an actor spawned this frame, its
        // attributes not worked out) is left to be captured as it applies.
        let Some(amount) = amount else {
            continue;
        };
        let snapshot = Snapshot {
            attribute: capture.attribute,
            value: capture.value,
            tags: capture.tags.clone(),
            amount,
        };
        match spec.snapshot.iter_mut().find(|taken| {
            taken.attribute == snapshot.attribute
                && taken.value == snapshot.value
                && taken.tags == snapshot.tags
        }) {
            Some(taken) => *taken = snapshot,
            None => spec.snapshot.push(snapshot),
        }
    }
    spec
}
