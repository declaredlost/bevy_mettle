//! Keeping actors' attributes: bases synced in, values worked out, and what
//! changed told.

use bevy::prelude::*;
use bevy_mettle_core::tag::OwnedTags;

use super::AttributeState;
use crate::registry::{AttributeChanged, AttributeRegistry};
use crate::{Attribute, Base, Current};

/// Syncs each actor's changed `Base<A>` into its state.
pub(crate) fn sync_base<A: Attribute>(
    mut actors: Query<(&Base<A>, &mut AttributeState), Changed<Base<A>>>,
) {
    for (base, mut state) in &mut actors {
        state.set_base(A::ID, base.get());
    }
}

/// Takes `A` off an actor as its `Base<A>` is taken off: its value forgotten,
/// and its `Current<A>` taken off too.
pub(crate) fn remove_base<A: Attribute>(
    event: On<Remove<Base<A>>>,
    mut actors: Query<&mut AttributeState>,
    mut commands: Commands,
) {
    if let Ok(mut state) = actors.get_mut(event.entity) {
        state.remove(&A::ID);
    }
    commands.entity(event.entity).try_remove::<Current<A>>();
}

/// Actors whose attributes or owned tags changed.
type Touched = Or<(Changed<AttributeState>, Changed<OwnedTags>)>;

/// Works out the values of every actor whose attributes or owned tags changed,
/// marking its state changed only where one did.
pub(crate) fn refresh(
    registry: Res<AttributeRegistry>,
    mut actors: Query<(&mut AttributeState, &OwnedTags), Touched>,
) {
    for (mut state, owned) in &mut actors {
        if state
            .bypass_change_detection()
            .refresh(&registry, owned.counts())
        {
            state.set_changed();
        }
    }
}

/// Writes each actor's `Current<A>` where its value changed, and tells each one
/// that did: not its first, which it had none before.
pub(crate) fn publish<A: Attribute>(
    mut commands: Commands,
    mut actors: Query<(Entity, &AttributeState, &mut Current<A>), Changed<AttributeState>>,
) {
    let id = A::ID;
    for (entity, state, mut current) in &mut actors {
        let Some(new) = state.current(&id) else {
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
