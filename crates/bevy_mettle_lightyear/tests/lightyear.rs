//! mettle over lightyear, end to end: a server and a client in one
//! process, over crossbeam. The client predicts its mage's fireball; the
//! server decides; lightyear carries the request, the reply, the replicas
//! and the mana.

use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;

use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::time::TimeUpdateStrategy;
use bevy_mettle_ability::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::curve::CurveTable;
use bevy_mettle_core::tag::{OwnedTags, Tag};
use bevy_mettle_effect::prelude::*;
use bevy_mettle_lightyear::{MettleNetApp, MettleNetPlugin};
use bevy_replicon::shared::server_entity_map::ServerEntityMap;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::netcode::Key;
use lightyear::netcode::auth::Authentication;
use lightyear::prelude::client::{ClientPlugins, NetcodeClient};
use lightyear::prelude::server::{NetcodeServer, ServerPlugins, Start};
use lightyear::prelude::*;

const TICK: Duration = Duration::from_millis(10);

/// Where the server is, as far as netcode is told.
const SERVER: SocketAddr = SocketAddr::new(core::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);

#[derive(Attribute)]
#[attribute(base = 30.0, min = 0.0)]
struct Mana;

#[derive(Component, Default)]
struct Cast;

/// What both sides have: mettle, fireball, and what's carried.
fn protocol(app: &mut App, role: NetRole) {
    app.insert_resource(role)
        .add_attribute::<Mana>()
        .add_plugins((EffectPlugin::default(), AbilityPlugin))
        .add_behaviour::<Cast>("cast")
        .add_plugins(MettleNetPlugin)
        .replicate_attribute::<Mana>();
    // A game replicating its items' modifiers, as they're rolled.
    app.component::<Modifiers>().replicate();
    app.component::<ModifierOf>().replicate();
    let tags: Vec<Tag> = ["state", "state.stunned", "cooldown", "cooldown.fireball"]
        .into_iter()
        .map(|id| Tag {
            id: Id::from_static(id),
        })
        .collect();
    let tags = TagTree::build(&tags).unwrap();
    let world = app.world_mut();
    let effects = EffectRegistry::build(
        &[
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            Effect::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
        ],
        &tags,
        &CurveTable::default(),
        world.resource::<AttributeRegistry>(),
        &EffectCode::new(),
    )
    .unwrap();
    let fireball = Ability::new("fireball", "cast")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .blocked_by("state.stunned");
    let abilities = AbilityRegistry::build(
        &[fireball],
        &[],
        &tags,
        &effects,
        world.resource::<Behaviours>(),
    )
    .unwrap();
    world.insert_resource(effects);
    world.insert_resource(abilities);
}

/// A server and a client, connected.
struct Net {
    server: App,
    client: App,
    /// The client's link, on the server.
    link: Entity,
    now: Instant,
}

impl Net {
    fn new() -> Self {
        let mut server = App::new();
        server.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            ServerPlugins {
                tick_duration: TICK,
            },
            RoomPlugin,
        ));
        protocol(&mut server, NetRole::Server);
        let endpoint = server
            .world_mut()
            .spawn(NetcodeServer::new(
                lightyear::netcode::server_plugin::NetcodeConfig {
                    server_addr_check: false,
                    ..default()
                },
            ))
            .id();
        let (client_io, server_io) = CrossbeamIo::new_pair();
        let link = server
            .world_mut()
            .spawn((
                LinkOf { endpoint },
                Link::default(),
                PeerAddr(SocketAddr::new(Ipv4Addr::LOCALHOST.into(), 1)),
                Linked,
                server_io,
                ReplicationSender,
                ReplicationReceiver,
                PingManager::new(PingConfig {
                    ping_interval: Duration::default(),
                }),
            ))
            .id();
        let mut client = App::new();
        client.add_plugins((
            MinimalPlugins,
            StatesPlugin,
            ClientPlugins {
                tick_duration: TICK,
            },
        ));
        protocol(&mut client, NetRole::Client);
        client.finish();
        client.cleanup();
        let peer = client
            .world_mut()
            .spawn((
                Client,
                NetcodeClient::new(
                    Authentication::Manual {
                        server_addr: SERVER,
                        protocol_id: 0,
                        private_key: Key::default(),
                        client_id: 0,
                    },
                    lightyear::netcode::client_plugin::NetcodeConfig::default(),
                )
                .unwrap(),
                ReplicationSender,
                ReplicationReceiver,
                PingManager::new(PingConfig {
                    ping_interval: Duration::default(),
                }),
                client_io,
            ))
            .id();
        server.finish();
        server.cleanup();
        let now = Instant::now();
        for app in [&mut server, &mut client] {
            app.world_mut()
                .resource_mut::<Time<Real>>()
                .update_with_instant(now);
        }
        server.world_mut().trigger(Start { entity: endpoint });
        client.world_mut().trigger(Connect { entity: peer });
        let mut net = Self {
            server,
            client,
            link,
            now,
        };
        for _ in 0..50 {
            if net.client.world().get::<Connected>(peer).is_some() {
                break;
            }
            net.step(1);
        }
        assert!(
            net.client.world().get::<Connected>(peer).is_some(),
            "connected"
        );
        net
    }

    /// Runs `n` ticks on both sides.
    fn step(&mut self, n: usize) {
        for _ in 0..n {
            self.now += TICK;
            // Lightyear reads real time too, as its own stepper does.
            std::thread::sleep(TICK);
            for app in [&mut self.client, &mut self.server] {
                app.insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
                app.update();
            }
        }
    }

    /// A mage on the server, `controlled` by the client if so, granted
    /// fireball, and replicated: it, and its mage on the client.
    fn mage(&mut self, controlled: bool) -> (Entity, Entity) {
        let mut mage = self.server.world_mut().spawn((
            Base::<Mana>::default(),
            Replicate::to_clients(NetworkTarget::All),
        ));
        if controlled {
            mage.insert(ControlledBy {
                owner: self.link,
                lifetime: Lifetime::default(),
            });
        }
        let there = mage.id();
        self.server
            .world_mut()
            .grant_ability(there, "fireball", 1.0);
        for _ in 0..50 {
            self.step(1);
            let here = self
                .client
                .world()
                .resource::<ServerEntityMap>()
                .to_client()
                .get(&there)
                .copied();
            if let Some(here) = here
                && self
                    .client
                    .world()
                    .get::<Abilities>(here)
                    .is_some_and(|abilities| {
                        abilities.iter().any(|ability| {
                            self.client.world().get::<GrantedAbility>(ability).is_some()
                        })
                    })
            {
                return (there, here);
            }
        }
        let map = self
            .client
            .world()
            .resource::<ServerEntityMap>()
            .to_client()
            .len();
        let replicas = self
            .server
            .world_mut()
            .query::<&AbilityReplica>()
            .iter(self.server.world())
            .count();
        let link: Vec<String> = self
            .server
            .world()
            .inspect_entity(self.link)
            .unwrap()
            .map(|(_, info)| info.name().to_string())
            .collect();
        panic!(
            "the mage and its fireball didn't replicate: {map} mapped, {replicas} replicas; link: {link:?}"
        );
    }

    fn mana(app: &App, mage: Entity) -> f32 {
        app.world()
            .value_at(mage, &Mana::ID, &AttributeContext::new())
            .unwrap()
    }

    fn effects(app: &App, mage: Entity) -> Vec<(bool, bool)> {
        app.world()
            .get::<ActiveEffects>(mage)
            .map(|effects| effects.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|effect| {
                (
                    app.world()
                        .get::<bevy_mettle_effect::PredictedUnder>(effect)
                        .is_some(),
                    app.world()
                        .get::<bevy_mettle_effect::Proxy>(effect)
                        .is_some(),
                )
            })
            .collect()
    }
}

#[test]
fn a_client_predicts_and_the_servers_own_replaces_it() {
    let mut net = Net::new();
    let (there, here) = net.mage(true);
    net.client
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    net.step(1);
    assert!(
        (Net::mana(&net.client, here) - 10.0).abs() < 1e-4,
        "paid, predicted"
    );
    assert_eq!(
        Net::effects(&net.client, here),
        [(true, false)],
        "a predicted cooldown"
    );
    net.step(20);
    assert!(
        (Net::mana(&net.server, there) - 10.0).abs() < 1e-4,
        "the server agreed"
    );
    assert!(
        (Net::mana(&net.client, here) - 10.0).abs() < 1e-4,
        "paid once"
    );
    assert_eq!(
        Net::effects(&net.client, here),
        [(false, true)],
        "the server's, shown"
    );
}

#[test]
fn a_rejected_prediction_is_undone() {
    let mut net = Net::new();
    let (there, here) = net.mage(true);
    net.server
        .world_mut()
        .get_mut::<OwnedTags>(there)
        .unwrap()
        .add(&Id::from_static("state.stunned"));
    net.client
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    net.step(20);
    assert!(
        (Net::mana(&net.server, there) - 30.0).abs() < 1e-4,
        "refused"
    );
    assert!(
        (Net::mana(&net.client, here) - 30.0).abs() < 1e-4,
        "mana back"
    );
    assert_eq!(
        Net::effects(&net.client, here),
        [],
        "the cooldown taken off"
    );
}

#[test]
fn a_client_cant_ask_for_a_mage_it_doesnt_control() {
    let mut net = Net::new();
    let (there, here) = net.mage(false);
    net.client
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    net.step(20);
    assert!(
        (Net::mana(&net.server, there) - 30.0).abs() < 1e-4,
        "ignored"
    );
    assert!(
        (Net::mana(&net.client, here) - 30.0).abs() < 1e-4,
        "rejected, and undone"
    );
    assert_eq!(Net::effects(&net.client, here), []);
}

#[test]
fn an_items_modifiers_replicate_and_count_on_the_client() {
    let mut net = Net::new();
    let (there, here) = net.mage(true);
    let modifiers = Modifiers(vec![
        Modifier::flat::<Mana>(5.0),
        Modifier::additive::<Mana>(1.0).owner_has("state.stunned"),
        Modifier::flat::<Mana>(100.0).local(),
    ]);
    let ring = net
        .server
        .world_mut()
        .spawn((
            ModifierOf(there),
            modifiers.clone(),
            Replicate::to_clients(NetworkTarget::All),
        ))
        .id();
    net.step(20);
    let mapped = net
        .client
        .world()
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&ring)
        .copied()
        .expect("the ring replicated");
    assert_eq!(
        net.client.world().get::<Modifiers>(mapped),
        Some(&modifiers),
        "every field, through lightyear's binary format"
    );
    assert!(
        (Net::mana(&net.client, here) - 35.0).abs() < 1e-4,
        "and counting"
    );
}
