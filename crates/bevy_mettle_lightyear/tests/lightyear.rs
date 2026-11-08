//! mettle over lightyear, end to end: a server and a client in one process,
//! over crossbeam. The client predicts its mage's fireball; the server decides;
//! lightyear carries the request, the reply, the replicas and the mana.

use core::net::{Ipv4Addr, SocketAddr};
use core::time::Duration;

use bevy::app::Plugins;
use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_harness::{TestApp, TestWorld};
use bevy_mettle_ability::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::prelude::*;
use bevy_mettle_effect::prelude::*;
use bevy_mettle_lightyear::prelude::*;
use bevy_replicon::shared::server_entity_map::ServerEntityMap;
use lightyear::crossbeam::CrossbeamIo;
use lightyear::netcode::Key;
use lightyear::netcode::auth::Authentication;
use lightyear::prelude::client::{ClientPlugins, NetcodeClient};
use lightyear::prelude::server::{NetcodeServer, ServerPlugins, Start};
use lightyear::prelude::*;
use serde::{Deserialize, Serialize};

const TICK: Duration = Duration::from_millis(10);

/// Where the server is, as far as netcode is told.
const SERVER: SocketAddr = SocketAddr::new(core::net::IpAddr::V4(Ipv4Addr::LOCALHOST), 5000);

#[derive(Attribute)]
#[attribute(base = 30.0, min = 0.0)]
struct Mana;

#[derive(Component, Default, Behaviour)]
struct Cast;

/// Where a shot is aimed: how far along. Sent from clients.
#[derive(
    Component,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    MapEntities
)]
struct Aim(f32);

/// A shot being fired: aimed.
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct Shoot;

/// How far a shot reaches, as this side knows: the server, better.
#[derive(Resource)]
struct Reach(f32);

fn in_reach(In(check): In<InputCheck<Aim>>, reach: Res<Reach>) -> bool {
    check.input.0.abs() <= reach.0
}

/// What both sides have: mettle, fireball, and what's carried.
fn protocol(app: &mut App, role: NetRole, replicas: Replicas) {
    app.insert_resource(role)
        .add_attribute::<Mana>()
        .add_plugins((EffectPlugin::default(), AbilityPlugin))
        .add_behaviour::<Cast>()
        .add_behaviour::<Shoot>()
        .check_input::<Aim, _>(in_reach)
        .insert_resource(Reach(if role == NetRole::Server { 10.0 } else { 100.0 }))
        .add_plugins(MettleNetPlugin { replicas })
        .replicate_attribute::<Mana>()
        .replicate_input::<Aim>();
    // A game replicating its items' modifiers, as they're rolled.
    app.component::<Modifiers>().replicate();
    app.component::<ModifierOf>().replicate();
    let tags: Vec<Tag> = [
        "state",
        "state.stunned",
        "cooldown",
        "cooldown.fireball",
        "cue",
        "cue.cooling",
        "cue.hit",
    ]
    .into_iter()
    .map(|id| Tag {
        id: Id::from_static(id),
    })
    .collect();
    let tags = TagTree::build(&tags).unwrap();
    let world = app.world_mut();
    let effects = EffectRegistry::check(
        &[
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))
                .cue("cue.hit"),
            Effect::lasting("fireball.cooldown", 1.0)
                .grant("cooldown.fireball")
                .cue("cue.cooling"),
        ],
        EffectChecks {
            tags: &tags,
            curves: &CurveTable::default(),
            attributes: world.resource::<AttributeRegistry>(),
            code: &EffectCode::new(),
        },
    )
    .unwrap();
    let fireball = Ability::new("fireball", "cast")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .blocked_by("state.stunned")
        .cancel_if(Condition::owner_has("state.stunned"));
    let shot = Ability::new("shot", "shoot").cost("fireball.cost");
    let abilities = AbilityRegistry::check(
        &[fireball, shot],
        &[],
        AbilityChecks {
            tags: &tags,
            effects: &effects,
            behaviours: world.resource::<Behaviours>(),
        },
    )
    .unwrap();
    world.insert_resource(effects);
    world.insert_resource(abilities);
}

/// The app of one side, `role`, on `plugins`: lightyear's, with mettle.
fn side<M>(role: NetRole, replicas: Replicas, plugins: impl Plugins<M>) -> TestApp {
    TestApp::builder()
        .plugins((StatesPlugin, plugins))
        .with(move |app| protocol(app, role, replicas))
        .fixed_hz(1.0 / TICK.as_secs_f64())
        .build()
}

/// A server, as `"server"`, and a client, as `"client"`, connected over
/// crossbeam: the server ticked first, then the client.
fn net() -> TestWorld {
    net_with(Replicas::Owner)
}

/// As [`net`], replicas sent as `replicas` says.
fn net_with(replicas: Replicas) -> TestWorld {
    let mut server = side(
        NetRole::Server,
        replicas,
        (
            ServerPlugins {
                tick_duration: TICK,
            },
            RoomPlugin,
        ),
    );
    let endpoint = server.spawn(NetcodeServer::new(
        lightyear::netcode::server_plugin::NetcodeConfig {
            server_addr_check: false,
            ..default()
        },
    ));
    let (client_io, server_io) = CrossbeamIo::new_pair();
    server.spawn((
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
    ));
    let mut client = side(
        NetRole::Client,
        replicas,
        ClientPlugins {
            tick_duration: TICK,
        },
    );
    let peer = client.spawn((
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
    ));
    server.trigger(Start { entity: endpoint });
    client.trigger(Connect { entity: peer });
    let mut world = TestWorld::new()
        .with("server", server)
        .with("client", client);
    world
        .until(|world| world["client"].entity(peer).has::<Connected>())
        .describe("the client connects")
        .within_ticks(50);
    world
}

/// The client's link, on the server.
fn link(world: &mut TestWorld) -> Entity {
    world["server"].query::<Entity, With<LinkOf>>().single()
}

/// What `there`, on the server, is on the client, if it's replicated.
fn mapped(client: &TestApp, there: Entity) -> Option<Entity> {
    client
        .resource::<ServerEntityMap>()
        .to_client()
        .get(&there)
        .copied()
}

/// A mage on the server, `controlled` by the client if so, granted
/// fireball, and replicated: it, and its mage on the client.
fn mage(world: &mut TestWorld, controlled: bool) -> (Entity, Entity) {
    let link = link(world);
    let server = &mut world["server"];
    let there = server.spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    if controlled {
        server.entity_mut(there).insert(ControlledBy {
            owner: link,
            lifetime: Lifetime::default(),
        });
    }
    server.world_mut().grant_ability(there, "fireball", 1.0);
    world
        .until(|world| {
            let client = &world["client"];
            mapped(client, there).is_some_and(|here| {
                client
                    .entity(here)
                    .try_get::<Abilities>()
                    .is_some_and(|abilities| {
                        abilities
                            .iter()
                            .any(|ability| client.entity(ability).has::<GrantedAbility>())
                    })
            })
        })
        .describe("the mage and its fireball replicate")
        .within_ticks(50);
    (there, mapped(&world["client"], there).unwrap())
}

/// `mage`'s mana, as `app` reads it.
fn mana(app: &TestApp, mage: Entity) -> f32 {
    app.world()
        .value_at(mage, &Mana::ID, &AttributeContext::new())
        .unwrap()
}

/// `mage`'s effects, as `app` has them: whether each is predicted, and
/// whether it's a proxy of the server's.
fn effects(app: &TestApp, mage: Entity) -> Vec<(bool, bool)> {
    app.entity(mage)
        .try_get::<ActiveEffects>()
        .into_iter()
        .flat_map(ActiveEffects::iter)
        .map(|effect| {
            let effect = app.entity(effect);
            (
                effect.has::<bevy_mettle_effect::net::PredictedUnder>(),
                effect.has::<bevy_mettle_effect::net::Proxy>(),
            )
        })
        .collect()
}

/// Whether `a` and `b` are the same mana, to a float's error.
fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
}

#[test]
fn a_client_predicts_and_the_servers_own_replaces_it() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world.tick();
    assert!(same(mana(&world["client"], here), 10.0), "paid, predicted");
    assert_eq!(
        effects(&world["client"], here),
        [(true, false)],
        "a predicted cooldown"
    );
    world.ticks(20);
    assert!(
        same(mana(&world["server"], there), 10.0),
        "the server agreed"
    );
    assert!(same(mana(&world["client"], here), 10.0), "paid once");
    assert_eq!(
        effects(&world["client"], here),
        [(false, true)],
        "the server's, shown"
    );
}

#[test]
fn a_rejected_prediction_is_undone() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    world["server"]
        .entity_mut(there)
        .get_mut::<OwnedTags>()
        .unwrap()
        .add(&Id::from_static("state.stunned"));
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 30.0), "refused");
    assert!(same(mana(&world["client"], here), 30.0), "mana back");
    assert_eq!(
        effects(&world["client"], here),
        [],
        "the cooldown taken off"
    );
}

#[test]
fn a_client_cant_ask_for_a_mage_it_doesnt_control() {
    // Its replicas sent to every client, so this one sees the fireball.
    let mut world = net_with(Replicas::Everyone);
    let (there, here) = mage(&mut world, false);
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 30.0), "ignored");
    assert!(
        same(mana(&world["client"], here), 30.0),
        "rejected, and undone"
    );
    assert_eq!(effects(&world["client"], here), []);
}

#[test]
fn an_items_modifiers_replicate_and_count_on_the_client() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let modifiers = Modifiers(vec![
        Modifier::flat::<Mana>(5.0),
        Modifier::additive::<Mana>(1.0).owner_has("state.stunned"),
        Modifier::flat::<Mana>(100.0).local(),
    ]);
    let ring = world["server"].spawn((
        ModifierOf(there),
        modifiers.clone(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world.ticks(20);
    let client = &world["client"];
    let ring = mapped(client, ring).expect("the ring replicated");
    client.entity(ring).assert_eq(modifiers);
    assert!(same(mana(client, here), 35.0), "and counting");
}

#[test]
fn a_confirm_waits_for_the_servers_state_as_of_it() {
    let mut world = net();
    // Replication lags the reply: the server's state goes out every 50ms.
    world["server"].insert_resource(ReplicationMetadata::new(Duration::from_millis(50)));
    let (there, here) = mage(&mut world, true);
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    for step in 0..30 {
        world.tick();
        let mana = mana(&world["client"], here);
        assert!(
            same(mana, 10.0),
            "paid once, and only once, all along: {mana} at step {step}"
        );
    }
    assert!(same(mana(&world["server"], there), 10.0));
}

#[test]
fn a_shown_effects_source_is_the_clients_own() {
    let mut world = net();
    let (_, here) = mage(&mut world, true);
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world.ticks(20);
    let client = &world["client"];
    let shown = client
        .entity(here)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        client.entity(shown).get::<ActiveEffect>().spec().source,
        Some(here),
        "mapped from the server's"
    );
}

#[test]
fn an_actor_not_replicated_has_its_effects_kept_back_too() {
    let mut world = net();
    let (_, here) = mage(&mut world, true);
    let server = &mut world["server"];
    let hidden = server.spawn(Base::<Mana>::default());
    server.tick();
    server
        .world_mut()
        .apply_effect(hidden, EffectSpec::new("fireball.cooldown"));
    world.ticks(20);
    world["client"].query::<&EffectReplica, ()>().assert_len(0);
    world["client"].entity(here).assert_has::<Abilities>();
}

/// `here`'s mana as its `Current` says.
fn current(app: &TestApp, here: Entity) -> f32 {
    app.entity(here).get::<Current<Mana>>().get()
}

#[test]
fn a_client_shows_the_servers_override_whatever_order_its_sources_came_in() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let ring = |world: &mut TestWorld, value: f32| {
        world["server"].spawn((
            ModifierOf(there),
            Modifiers(vec![Modifier::overriding::<Mana>(value)]),
            Replicate::to_clients(NetworkTarget::All),
        ))
    };
    let first = ring(&mut world, 5.0);
    world.ticks(10);
    ring(&mut world, 15.0);
    world.ticks(10);
    assert!(
        same(mana(&world["server"], there), 15.0),
        "the last applied"
    );
    // On the client, the first ring comes (again) after the second.
    let first = mapped(&world["client"], first).unwrap();
    world["client"].entity_mut(first).insert(ModifierOf(here));
    world.ticks(2);
    let client = &world["client"];
    let read = AttributeContext::new().tag("state");
    let local = client.world().value_at(here, &Mana::ID, &read).unwrap();
    assert!(same(local, 5.0), "its own fold would say {local}");
    assert!(same(mana(client, here), 15.0), "but it shows the server's");
    assert!(same(current(client, here), 15.0));
}

#[test]
fn a_prediction_shows_until_answered_then_the_served_value() {
    let mut world = net();
    world["server"].insert_resource(ReplicationMetadata::new(Duration::from_millis(50)));
    let (there, here) = mage(&mut world, true);
    // A source only the server has: what the client can't work out itself.
    world["server"].spawn((
        ModifierOf(there),
        Modifiers(vec![Modifier::flat::<Mana>(100.0)]),
    ));
    world.ticks(20);
    assert!(same(current(&world["client"], here), 130.0), "served");
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    let mut answered = false;
    for step in 0..30 {
        world.tick();
        let client = &world["client"];
        let shown = (mana(client, here), current(client, here));
        if same(shown.0, 110.0) && same(shown.1, 110.0) {
            answered = true;
        } else {
            assert!(
                !answered && same(shown.0, 10.0) && same(shown.1, 10.0),
                "the prediction, then the served value, nothing between: {shown:?} at {step}"
            );
        }
    }
    assert!(answered, "served after the answer");
}

/// A mage on the server, controlled by the client, granted the shot: it, and
/// its mage on the client, with the shot replicated.
fn marksman(world: &mut TestWorld) -> (Entity, Entity) {
    let link = link(world);
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: link,
            lifetime: Lifetime::default(),
        },
    ));
    world["server"]
        .world_mut()
        .grant_ability(there, "shot", 1.0);
    world
        .until(|world| {
            let client = &world["client"];
            mapped(client, there).is_some_and(|here| {
                client
                    .entity(here)
                    .try_get::<Abilities>()
                    .is_some_and(|abilities| {
                        abilities
                            .iter()
                            .any(|ability| client.entity(ability).has::<GrantedAbility>())
                    })
            })
        })
        .describe("the marksman and its shot replicate")
        .within_ticks(50);
    (there, mapped(&world["client"], there).unwrap())
}

/// `actor`'s shot, as `app` has it.
fn shot_of(app: &TestApp, actor: Entity) -> Entity {
    app.entity(actor).get::<Abilities>().iter().next().unwrap()
}

#[test]
fn a_shot_is_sent_with_its_aim_and_the_server_fires_the_same() {
    let mut world = net();
    let (there, here) = marksman(&mut world);
    world["client"]
        .world_mut()
        .activate_ability_with::<Shoot>(here, "shot", Aim(4.0))
        .unwrap();
    world
        .until(|world| world["server"].count::<With<Shoot>>() == 1)
        .describe("the server fires")
        .within_ticks(20);
    let server = &world["server"];
    server.entity(shot_of(server, there)).assert_eq(Aim(4.0));
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 10.0));
    assert!(same(mana(&world["client"], here), 10.0), "paid once");
}

#[test]
fn a_shot_aimed_past_what_the_server_knows_is_undone() {
    let mut world = net();
    let (there, here) = marksman(&mut world);
    world["client"]
        .world_mut()
        .activate_ability_with::<Shoot>(here, "shot", Aim(40.0))
        .unwrap();
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 30.0), "never fired");
    assert!(same(mana(&world["client"], here), 30.0), "mana back");
    world["client"].query::<(), With<Shoot>>().assert_len(0);
}

#[test]
fn a_shot_aimed_again_past_reach_is_cancelled_on_both_sides() {
    let mut world = net();
    let (there, here) = marksman(&mut world);
    let shooting = world["client"]
        .world_mut()
        .activate_ability_with::<Shoot>(here, "shot", Aim(4.0))
        .unwrap();
    world.ticks(20);
    world["client"]
        .world_mut()
        .set_input::<Shoot>(shooting, Aim(6.0))
        .unwrap();
    world
        .until(|world| {
            let server = &world["server"];
            server
                .entity(shot_of(server, there))
                .try_get::<Aim>()
                .is_some_and(|aim| *aim == Aim(6.0))
        })
        .describe("the server takes the new aim")
        .within_ticks(20);
    world["client"]
        .world_mut()
        .set_input::<Shoot>(shooting, Aim(60.0))
        .unwrap();
    world
        .until(|world| {
            world["server"].count::<With<Shoot>>() == 0
                && world["client"].count::<With<Shoot>>() == 0
        })
        .describe("both sides cancel the shot")
        .within_ticks(20);
}

/// The tags of the lasting cues `app` shows on `actor`.
fn cues(app: &TestApp, actor: Entity) -> Vec<String> {
    app.entity(actor)
        .try_get::<ActiveCues>()
        .map(|cues| cues.iter().collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|cue| app.entity(cue).get::<ActiveCue>().tag.to_string())
        .collect()
}

#[test]
fn another_clients_actor_shows_its_cues_but_not_its_effects() {
    let mut world = net();
    // No client controls it, so its replicas go to none: only the actor
    // itself is waited for.
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world["server"]
        .world_mut()
        .grant_ability(there, "fireball", 1.0)
        .unwrap();
    world
        .until(|world| mapped(&world["client"], there).is_some())
        .describe("the actor replicates")
        .within_ticks(50);
    let here = mapped(&world["client"], there).unwrap();
    world.ticks(10);
    world["client"].entity(here).assert_lacks::<Abilities>();
    world["client"].watch_triggered::<CueExecuted>();
    let server = world["server"].world_mut();
    server.apply_effect(there, EffectSpec::new("fireball.cooldown"));
    server.apply_effect(there, EffectSpec::new("fireball.cost"));
    world
        .until(|world| cues(&world["client"], here) == ["cue.cooling"])
        .describe("the client shows the cooldown's cue")
        .within_ticks(20);
    world.ticks(10);
    let client = &mut world["client"];
    assert_eq!(effects(client, here), [], "its effects stay the server's");
    let played = client.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1, "the hit, once");
    assert_eq!(played[0].tag.as_str(), "cue.hit");
    assert!(same(played[0].magnitude, -20.0));
}

#[test]
fn a_one_shot_a_client_predicted_plays_once() {
    let mut world = net();
    let (_, here) = mage(&mut world, true);
    world["client"].watch_triggered::<CueExecuted>();
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world.ticks(30);
    let played: Vec<_> = world["client"]
        .read_triggered::<CueExecuted>()
        .into_iter()
        .filter(|executed| executed.tag.as_str() == "cue.hit")
        .collect();
    assert_eq!(
        played.len(),
        1,
        "predicted, and the server's not played again"
    );
    assert!(played[0].activation.is_some());
}

#[test]
fn a_client_coming_to_see_an_actor_joins_its_cues() {
    let mut world = net();
    world["client"].watch_triggered::<CueAdded>();
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world["server"]
        .world_mut()
        .apply_effect(there, EffectSpec::new("fireball.cooldown"));
    world
        .until(|world| {
            mapped(&world["client"], there)
                .is_some_and(|here| cues(&world["client"], here) == ["cue.cooling"])
        })
        .describe("the client shows the cooldown's cue")
        .within_ticks(20);
    world["client"].assert_triggered::<CueAdded>(|added| added.joined);
}

/// Whether `app` has a granted ability on `actor`.
fn has_abilities(app: &TestApp, actor: Entity) -> bool {
    app.entity(actor)
        .try_get::<Abilities>()
        .is_some_and(|abilities| {
            abilities
                .iter()
                .any(|ability| app.entity(ability).has::<GrantedAbility>())
        })
}

#[test]
fn replicas_follow_control_as_it_comes_and_goes() {
    let mut world = net();
    let link = link(&mut world);
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world["server"]
        .world_mut()
        .grant_ability(there, "fireball", 1.0)
        .unwrap();
    world
        .until(|world| mapped(&world["client"], there).is_some())
        .describe("the actor replicates")
        .within_ticks(50);
    let here = mapped(&world["client"], there).unwrap();
    world.ticks(10);
    assert!(
        !has_abilities(&world["client"], here),
        "no one controls it yet"
    );

    // The client comes to control it: its fireball follows.
    world["server"].entity_mut(there).insert(ControlledBy {
        owner: link,
        lifetime: Lifetime::default(),
    });
    world
        .until(|world| has_abilities(&world["client"], here))
        .describe("the fireball reaches its new owner")
        .within_ticks(50);

    // Control goes: so does the fireball.
    world["server"].entity_mut(there).remove::<ControlledBy>();
    world
        .until(|world| !has_abilities(&world["client"], here))
        .describe("the fireball leaves the client that lost control")
        .within_ticks(50);
}

#[test]
fn owner_only_replicas_go_and_come_back_with_their_actor() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    world["server"]
        .world_mut()
        .apply_effect(there, EffectSpec::new("fireball.cooldown"));
    world
        .until(|world| effects(&world["client"], here).len() == 1)
        .describe("the cooldown reaches its owner")
        .within_ticks(30);

    // The actor leaves the client's view: its replicas go with it.
    world["server"]
        .entity_mut(there)
        .insert(Replicate::to_clients(NetworkTarget::None));
    world
        .until(|world| {
            let client = &mut world["client"];
            client.count::<With<EffectReplica>>() == 0
                && client.count::<With<AbilityReplica>>() == 0
        })
        .describe("no replica outlives its actor leaving view")
        .within_ticks(30);

    // It comes back: so do they, on it.
    world["server"]
        .entity_mut(there)
        .insert(Replicate::to_clients(NetworkTarget::All));
    world
        .until(|world| {
            mapped(&world["client"], there).is_some_and(|here| {
                effects(&world["client"], here).len() == 1 && has_abilities(&world["client"], here)
            })
        })
        .describe("its replicas come back on it")
        .within_ticks(50);
}

/// Whether `app` has `actor`'s fireball active.
fn casting(app: &TestApp, actor: Entity) -> bool {
    app.entity(actor)
        .try_get::<Abilities>()
        .is_some_and(|abilities| {
            abilities
                .iter()
                .any(|ability| app.entity(ability).has::<ActiveAbility>())
        })
}

#[test]
fn a_cast_the_server_interrupts_ends_on_the_client() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world
        .until(|world| casting(&world["server"], there))
        .describe("the server casts too")
        .within_ticks(20);

    // Stunned on the server, which doesn't send its owned tags.
    world["server"]
        .entity_mut(there)
        .get_mut::<OwnedTags>()
        .unwrap()
        .add(&Id::from_static("state.stunned"));
    world
        .until(|world| !casting(&world["server"], there) && !casting(&world["client"], here))
        .describe("the cast ends on both sides")
        .within_ticks(20);
    assert!(
        same(mana(&world["client"], here), 10.0),
        "paid all the same"
    );
}

#[test]
fn a_cast_the_client_cancels_is_cancelled_on_the_server() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let cast = world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world
        .until(|world| casting(&world["server"], there))
        .describe("the server casts too")
        .within_ticks(20);
    world["client"].world_mut().cancel_ability(cast);
    world
        .until(|world| !casting(&world["server"], there))
        .describe("the server's cast is cancelled")
        .within_ticks(20);
}

#[test]
fn a_cast_cancelled_as_its_asked_for_never_runs_on_the_server() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    world["server"].watch_triggered::<AbilityEnded>();
    let cast = world["client"]
        .world_mut()
        .activate_ability(here, "fireball")
        .unwrap();
    world["client"].world_mut().cancel_ability(cast);
    world
        .until(|world| same(mana(&world["server"], there), 10.0))
        .describe("the server casts, and pays")
        .within_ticks(20);
    world
        .until(|world| !casting(&world["server"], there))
        .describe("the server's cast is cancelled, after it's asked for")
        .within_ticks(20);
    world["server"]
        .assert_triggered::<AbilityEnded>(|ended| ended.entity == there && ended.cancelled);
}
