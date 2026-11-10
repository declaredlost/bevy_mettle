//! Checking a client numbers content as its server does: what everything sent
//! by number counts on.

use bevy::prelude::*;
use bevy_mettle_ability::AbilityRegistry;
use bevy_mettle_core::curve::CurveRegistry;
use bevy_mettle_core::tag::TagRegistry;
use bevy_mettle_effect::EffectRegistry;
use bevy_mettle_effect::context::ContextDataRegistry;
use lightyear::prelude::server::ClientOf;
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

use crate::MettleChannel;

/// How a world numbers its content: each of its registries'
/// [`fingerprint`](bevy_mettle_core::registry::Registry::fingerprint).
///
/// The same on a client and its server when they number content the same, so
/// what one sends by number names the same thing to the other. With the types
/// effects' contexts carry, numbered in order of name. A server sends
/// its own to each client as it connects, and again as its content changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentFingerprint {
    /// The tags'.
    pub tags: u64,
    /// The curves'.
    pub curves: u64,
    /// The effects'.
    pub effects: u64,
    /// The abilities'.
    pub abilities: u64,
    /// The context data types'.
    pub context_data: u64,
}

impl ContentFingerprint {
    /// How `world`'s content is numbered: an empty registry for one it hasn't.
    #[must_use]
    pub fn of(world: &World) -> Self {
        fn fingerprint<R: Resource + Default + Clone>(
            world: &World,
            of: impl Fn(&R) -> u64,
        ) -> u64 {
            of(&world.get_resource::<R>().cloned().unwrap_or_default())
        }
        Self {
            tags: fingerprint(world, |tags: &TagRegistry| tags.fingerprint()),
            curves: fingerprint(world, |curves: &CurveRegistry| curves.fingerprint()),
            effects: fingerprint(world, |effects: &EffectRegistry| effects.fingerprint()),
            abilities: fingerprint(world, |abilities: &AbilityRegistry| abilities.fingerprint()),
            context_data: fingerprint(world, |data: &ContextDataRegistry| data.fingerprint()),
        }
    }
}

/// Triggered on a client whose content is numbered unlike its server's.
///
/// What either sends by number would name something else to the other.
///
/// Logged as an error too. Load the server's content (the same content,
/// reloaded the same way), or leave: nothing stops the numbers mismatching.
#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentMismatch {
    /// How the server numbers its content.
    pub server: ContentFingerprint,
    /// How the client numbers its own.
    pub client: ContentFingerprint,
}

/// On a client: how its server numbers content, as last told.
#[derive(Resource, Debug, Default)]
pub(crate) struct ServerContent(Option<ContentFingerprint>);

/// Whether this world's content changed, so its fingerprint may have.
pub(crate) fn content_changed(
    tags: Option<Res<TagRegistry>>,
    curves: Option<Res<CurveRegistry>>,
    effects: Option<Res<EffectRegistry>>,
    abilities: Option<Res<AbilityRegistry>>,
    data: Option<Res<ContextDataRegistry>>,
) -> bool {
    tags.is_some_and(|tags| tags.is_changed())
        || curves.is_some_and(|curves| curves.is_changed())
        || effects.is_some_and(|effects| effects.is_changed())
        || abilities.is_some_and(|abilities| abilities.is_changed())
        || data.is_some_and(|data| data.is_changed())
}

/// On a server: tells each client connecting how its content is numbered, and
/// every client again as it changes.
pub(crate) fn tell_content(world: &mut World) {
    let changed = world.run_system_cached(content_changed).unwrap_or_default();
    let fingerprint = ContentFingerprint::of(world);
    let mut links = world
        .query_filtered::<(&mut MessageSender<ContentFingerprint>, Ref<Connected>), With<ClientOf>>(
        );
    for (mut sender, connected) in links.iter_mut(world) {
        if changed || connected.is_added() {
            sender.send::<MettleChannel>(fingerprint);
        }
    }
}

/// On a client: hears how its server numbers content, and compares it with
/// its own, as either is told or changes.
pub(crate) fn compare_content(world: &mut World) {
    let mut receivers =
        world.query_filtered::<&mut MessageReceiver<ContentFingerprint>, With<Client>>();
    let heard: Option<ContentFingerprint> = receivers
        .iter_mut(world)
        .flat_map(|mut receiver| receiver.receive().collect::<Vec<_>>())
        .last();
    let changed = world.run_system_cached(content_changed).unwrap_or_default();
    let mut server = world.get_resource_or_init::<ServerContent>();
    if let Some(heard) = heard {
        server.0 = Some(heard);
    } else if !changed {
        return;
    }
    let Some(server) = server.0 else {
        return;
    };
    let client = ContentFingerprint::of(world);
    if client != server {
        error!(
            "this client numbers content unlike its server ({client:?}, the server's {server:?}): \
             what's sent by number names other things"
        );
        world.trigger(ContentMismatch { server, client });
    }
}
