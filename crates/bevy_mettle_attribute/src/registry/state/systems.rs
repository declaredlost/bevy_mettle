//! Keeping actors' attributes: bases synced in, values worked out, and what
//! changed told.

use bevy::prelude::*;
use bevy_mettle_core::tag::OwnedTags;

use super::AttributeState;
use crate::registry::{AttributeChanged, AttributeRegistry, Bound, OnLimit};
use crate::{Attribute, Base, Current};

/// Syncs each actor's changed `Base<A>` into its state.
pub(crate) fn sync_base<A: Attribute>(
    registry: Res<AttributeRegistry>,
    mut actors: Query<(&Base<A>, &mut AttributeState), Changed<Base<A>>>,
) {
    let Some(id) = registry.id_of::<A>() else {
        return;
    };
    for (base, mut state) in &mut actors {
        state.set_base(id, base.get());
    }
}

/// Takes `A` off an actor as its `Base<A>` is taken off: its value forgotten,
/// and its `Current<A>` taken off too.
pub(crate) fn remove_base<A: Attribute>(
    event: On<Remove<Base<A>>>,
    registry: Res<AttributeRegistry>,
    mut actors: Query<&mut AttributeState>,
    mut commands: Commands,
) {
    if let (Some(id), Ok(mut state)) = (registry.id_of::<A>(), actors.get_mut(event.entity)) {
        state.remove(id);
    }
    commands.entity(event.entity).try_remove::<Current<A>>();
}

/// Actors whose attributes or owned tags changed.
type Touched = Or<(Changed<AttributeState>, Changed<OwnedTags>)>;

/// Works out the values of every actor whose attributes or owned tags changed,
/// marking its state changed only where one did.
pub(crate) fn refresh(
    registry: Res<AttributeRegistry>,
    mut actors: Query<(&mut AttributeState, Ref<OwnedTags>), Touched>,
) {
    for (mut state, owned) in &mut actors {
        if state
            .bypass_change_detection()
            .refresh(&registry, owned.counts(), owned.is_changed())
        {
            state.set_changed();
        }
    }
}

/// Moves each actor's `Base<A>` as a limit of it that's another attribute
/// moved, as [`OnLimit`] says: kept within its limits, or at the same fraction
/// of its maximum. Not on an actor whose authority is another world: what it
/// serves moves.
pub(crate) fn follow_limits<A: Attribute>(
    registry: Res<AttributeRegistry>,
    mut actors: Query<(&mut Base<A>, &mut AttributeState, &OwnedTags), Changed<AttributeState>>,
) {
    let Some(info) = registry.id_of::<A>().and_then(|id| registry.get(id)) else {
        return;
    };
    let (id, min, max) = (info.id, info.min, info.max);
    let moved = |state: &AttributeState, bound: Option<Bound>| match bound {
        Some(Bound::Attribute(other)) => state.moved(other),
        _ => None,
    };
    for (mut base, mut state, owned) in &mut actors {
        if state.is_remote() {
            continue;
        }
        let max = moved(&state, max);
        if max.is_none() && moved(&state, min).is_none() {
            continue;
        }
        let mut value = base.get();
        if A::ON_LIMIT == OnLimit::Ratio
            && let Some((was, is)) = max
            && was > 0.0
        {
            value *= is / was;
        }
        let value = state.within_limits(&registry, owned.counts(), id, value);
        if value.to_bits() != base.get().to_bits() {
            base.set(value);
            state.set_base(id, value);
        }
    }
}

/// Works out again the values of every actor whose bases followed their
/// limits, so they're current the frame the limits moved.
pub(crate) fn rework(
    registry: Res<AttributeRegistry>,
    mut actors: Query<(&mut AttributeState, &OwnedTags), Changed<AttributeState>>,
) {
    for (mut state, owned) in &mut actors {
        if state.is_stale() {
            state
                .bypass_change_detection()
                .refresh(&registry, owned.counts(), false);
        }
    }
}

/// Writes each actor's `Current<A>` where its value changed, and tells each one
/// that did: not its first, which it had none before.
pub(crate) fn publish<A: Attribute>(
    registry: Res<AttributeRegistry>,
    mut commands: Commands,
    mut actors: Query<(Entity, &AttributeState, &mut Current<A>), Changed<AttributeState>>,
) {
    let Some(id) = registry.id_of::<A>() else {
        return;
    };
    for (entity, state, mut current) in &mut actors {
        let Some(new) = state.current(id) else {
            continue;
        };
        if current
            .worked_out()
            .is_some_and(|was| was.to_bits() == new.to_bits())
        {
            continue;
        }
        // Its first value worked out isn't a change: it had none before.
        let Some(old) = current.replace(new) else {
            continue;
        };
        if (old - new).abs() > 0.0 || old.is_nan() != new.is_nan() {
            commands.trigger(AttributeChanged::<A>::new(entity, old, new));
        }
    }
}
