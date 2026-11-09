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
use bevy_mettle_ability::data::NetExecution;
use bevy_mettle_ability::prelude::*;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_core::prelude::*;
use bevy_mettle_effect::context::ContextDataRegistry;
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

#[derive(Ability)]
struct Fireball;

#[derive(Ability)]
struct Shot;

/// Run by the server alone: a client's ask, sent and not run.
#[derive(Ability)]
struct Smite;

/// Run by the server, then by its owning client as the server says.
#[derive(Ability)]
struct Ward;

/// Run where it's activated, and never sent.
#[derive(Ability)]
struct Zoom;

/// Predicted, and sending `event.spark` under its activation as it starts.
#[derive(Ability)]
struct Flare;

/// Predicted, and triggered by `event.spark`.
#[derive(Ability)]
struct Spark;

/// Server-initiated, triggered by `event.beacon`, and flaring as it starts:
/// sending `event.spark` under its activation, the server's.
#[derive(Ability)]
struct Beacon;

/// Local only, triggered by `event.lens`.
#[derive(Ability)]
struct Lens;

/// Local only, triggered as `state.aiming` is added.
#[derive(Ability)]
struct Scope;

#[derive(Effect)]
#[effect(name = "fireball.cost")]
struct FireballCost;

#[derive(Effect)]
#[effect(name = "fireball.cooldown")]
struct FireballCooldown;

#[derive(Component, Default, Behaviour)]
struct Cast;

#[derive(Component, Default, Behaviour)]
struct Smiting;

#[derive(Component, Default, Behaviour)]
struct Warding;

#[derive(Component, Default, Behaviour)]
struct Zooming;

#[derive(Component, Default, Behaviour)]
struct Flaring;

#[derive(Component, Default, Behaviour)]
struct Sparking;

/// A flare, as it starts on either side: sends `event.spark` to its actor
/// under its activation.
fn flare(add: On<Add<Flaring>>, active: Query<&ActiveAbility>, mut commands: Commands) {
    let Ok(active) = active.get(add.entity) else {
        return;
    };
    let event =
        GameplayEvent::new(active.actor, TagKey::named("event.spark")).under(active.activation);
    commands.queue(move |world: &mut World| AbilityWorld::send_event(world, event));
}

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

/// The game's own on an effect's context: what a shot marked, and whether
/// it was a headshot. Added on both sides.
#[derive(
    ContextData,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    MapEntities
)]
struct Impact {
    #[entities]
    marked: Entity,
    headshot: bool,
}

/// The game's own too, added after [`Impact`].
#[derive(
    ContextData,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    MapEntities
)]
struct Wound {
    deep: bool,
}

/// What both sides have: mettle, fireball, and what's carried.
fn protocol(app: &mut App, role: NetRole, replicas: Replicas) {
    app.insert_resource(role)
        .add_attribute::<Mana>()
        .add_plugins((EffectPlugin::default(), AbilityPlugin))
        .add_behaviour::<Cast>()
        .add_behaviour::<Shoot>()
        .add_behaviour::<Smiting>()
        .add_behaviour::<Warding>()
        .add_behaviour::<Zooming>()
        .add_behaviour::<Flaring>()
        .add_behaviour::<Sparking>()
        .add_observer(flare)
        .check_input::<Aim, _>(in_reach)
        .add_ability::<Fireball>()
        .add_ability::<Shot>()
        .add_ability::<Smite>()
        .add_ability::<Ward>()
        .add_ability::<Zoom>()
        .add_ability::<Flare>()
        .add_ability::<Spark>()
        .add_ability::<Beacon>()
        .add_ability::<Lens>()
        .add_ability::<Scope>()
        .add_effect::<FireballCost>()
        .add_effect::<FireballCooldown>()
        .add_context_data::<Impact>()
        .add_context_data::<Wound>()
        .insert_resource(Reach(if role == NetRole::Server { 10.0 } else { 100.0 }))
        .add_plugins(MettleNetPlugin { replicas })
        .replicate_attribute::<Mana>()
        .replicate_input::<Aim>();
    let tags: Vec<TagKey> = [
        "state",
        "state.stunned",
        "cooldown",
        "cooldown.fireball",
        "cue",
        "cue.cooling",
        "cue.hit",
        "ability",
        "event",
        "event.ward",
        "event.spark",
        "event.beacon",
        "event.lens",
        "state.aiming",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect();
    let tags = TagRegistry::build(tags).unwrap();
    let world = app.world_mut();
    let effects = world
        .resource::<EffectRegistry>()
        .recheck(
            &[
                EffectData::instant("fireball.cost")
                    .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))
                    .cue("cue.hit"),
                EffectData::lasting("fireball.cooldown", 1.0)
                    .grant("cooldown.fireball")
                    .cue("cue.cooling"),
            ],
            EffectChecks {
                tags: &tags,
                curves: &CurveRegistry::default(),
                attributes: world.resource(),
                executions: world.resource(),
                calculations: world.resource(),
                routes: world.resource(),
            },
        )
        .unwrap();
    let abilities = world
        .resource::<AbilityRegistry>()
        .recheck(
            &abilities(),
            &[],
            AbilityChecks {
                tags: &tags,
                effects: &effects,
                behaviours: world.resource(),
            },
        )
        .unwrap();
    world.insert_resource(effects);
    world.insert_resource(abilities);
    world.insert_resource(tags);
}

/// Every ability both sides have, as data.
fn abilities() -> [AbilityData; 10] {
    let fireball = AbilityData::new("fireball", "cast")
        .tag("ability")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .blocked_by("state.stunned")
        .cancel_if(Condition::owner_has("state.stunned"));
    let shot = AbilityData::new("shot", "shoot").cost("fireball.cost");
    let smite = AbilityData::new("smite", "smiting")
        .cost("fireball.cost")
        .net(NetExecution::ServerOnly);
    let ward = AbilityData::new("ward", "warding")
        .cost("fireball.cost")
        .trigger(Trigger::Event(TagKey::named("event.ward")))
        .net(NetExecution::ServerInitiated);
    let zoom = AbilityData::new("zoom", "zooming").net(NetExecution::LocalOnly);
    let flare = AbilityData::new("flare", "flaring")
        .cost("fireball.cost")
        .blocked_by("state.stunned");
    let spark =
        AbilityData::new("spark", "sparking").trigger(Trigger::Event(TagKey::named("event.spark")));
    let beacon = AbilityData::new("beacon", "flaring")
        .trigger(Trigger::Event(TagKey::named("event.beacon")))
        .net(NetExecution::ServerInitiated);
    let lens = AbilityData::new("lens", "zooming")
        .trigger(Trigger::Event(TagKey::named("event.lens")))
        .net(NetExecution::LocalOnly);
    let scope = AbilityData::new("scope", "zooming")
        .trigger(Trigger::TagAdded(TagKey::named("state.aiming")))
        .net(NetExecution::LocalOnly);
    [
        fireball, shot, smite, ward, zoom, flare, spark, beacon, lens, scope,
    ]
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
    server.world_mut().grant_ability(there, Fireball);
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
        .value_at(
            mage,
            app.world().attribute_id::<Mana>().unwrap(),
            &AttributeContext::new(),
        )
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
        .activate_ability(here, Fireball)
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
        .add("state.stunned");
    world["client"]
        .world_mut()
        .activate_ability(here, Fireball)
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
        .activate_ability(here, Fireball)
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
    // The server's numbers: the client, adding the same attributes in the same
    // order, means the same by them.
    let id = world["server"].world().attribute_id::<Mana>().unwrap();
    let modifiers = Modifiers(vec![
        Modifier::flat(id, 5.0),
        Modifier::additive(id, 1.0).owner_has("state.stunned"),
        Modifier::flat(id, 100.0).local(),
    ]);
    let ring = world["server"].spawn((
        ModifierOf(there),
        modifiers.clone(),
        ModifiersReplica::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world.ticks(20);
    let client = &world["client"];
    let ring = mapped(client, ring).expect("the ring replicated");
    client.entity(ring).assert_eq(modifiers);
    assert!(same(mana(client, here), 35.0), "and counting");
}

#[test]
fn a_sources_modifiers_go_by_number_and_count_in_the_clients_own_fold() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let id = world["server"].world().attribute_id::<Mana>().unwrap();
    // Counted for reads tagged `state` alone: the client's own fold, not the
    // value the server sends.
    let ring = world["server"].spawn((
        ModifierOf(there),
        Modifiers(vec![Modifier::flat(id, 7.0).read_tagged("state")]),
        ModifiersReplica::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world.ticks(20);
    let state = world["server"]
        .resource::<TagRegistry>()
        .id("state")
        .unwrap();
    let client = &world["client"];
    let ring = mapped(client, ring).expect("the ring replicated");
    assert_eq!(
        client
            .entity(ring)
            .get::<ModifiersReplica>()
            .modifiers
            .iter()
            .map(|modifier| modifier.condition.clone())
            .collect::<Vec<_>>(),
        [Some(ConditionOver::ReadTagged(state))],
        "sent by number"
    );
    client.entity(ring).assert_eq(Modifiers(vec![
        Modifier::flat(id, 7.0).read_tagged("state"),
    ]));
    let read = AttributeContext::new().tag("state");
    let tagged = client.world().value_at(here, id, &read).unwrap();
    assert!(same(tagged, 37.0), "counting in a tagged read: {tagged}");
    let plain = client
        .world()
        .value_at(here, id, &AttributeContext::new().tag("cue"))
        .unwrap();
    assert!(same(plain, 30.0), "and not in another: {plain}");
}

#[test]
fn a_confirm_waits_for_the_servers_state_as_of_it() {
    let mut world = net();
    // Replication lags the reply: the server's state goes out every 50ms.
    world["server"].insert_resource(ReplicationMetadata::new(Duration::from_millis(50)));
    let (there, here) = mage(&mut world, true);
    world["client"]
        .world_mut()
        .activate_ability(here, Fireball)
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
        .activate_ability(here, Fireball)
        .unwrap();
    world.ticks(20);
    let client = &world["client"];
    let effects: Vec<Entity> = client.entity(here).get::<ActiveEffects>().iter().collect();
    // The cooldown alone lasts: the cost is instant.
    let [shown] = effects[..] else {
        panic!("the cooldown alone, shown: {effects:?}");
    };
    assert_eq!(
        client.entity(shown).get::<ActiveEffect>().id(),
        client
            .resource::<EffectRegistry>()
            .id_of::<FireballCooldown>()
            .unwrap(),
        "the cooldown"
    );
    assert_eq!(
        client
            .entity(shown)
            .get::<ActiveEffect>()
            .spec()
            .context
            .source,
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
    server.world_mut().apply_effect(hidden, FireballCooldown);
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
    let id = world["server"].world().attribute_id::<Mana>().unwrap();
    let ring = move |world: &mut TestWorld, value: f32| {
        world["server"].spawn((
            ModifierOf(there),
            Modifiers(vec![Modifier::overriding(id, value)]),
            ModifiersReplica::default(),
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
    let local = client.world().value_at(here, id, &read).unwrap();
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
    let id = world["server"].world().attribute_id::<Mana>().unwrap();
    world["server"].spawn((
        ModifierOf(there),
        Modifiers(vec![Modifier::flat(id, 100.0)]),
    ));
    world.ticks(20);
    assert!(same(current(&world["client"], here), 130.0), "served");
    world["client"]
        .world_mut()
        .activate_ability(here, Fireball)
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
    world["server"].world_mut().grant_ability(there, Shot);
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
        .activate_ability_with::<Shoot>(here, Shot, Aim(4.0))
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
        .activate_ability_with::<Shoot>(here, Shot, Aim(40.0))
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
        .activate_ability_with::<Shoot>(here, Shot, Aim(4.0))
        .unwrap()
        .ability();
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
        .grant_ability(there, Fireball)
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
    server.apply_effect(there, FireballCooldown);
    server.apply_effect(there, FireballCost);
    world
        .until(|world| cues(&world["client"], here) == ["cue.cooling"])
        .describe("the client shows the cooldown's cue")
        .within_ticks(20);
    world.ticks(10);
    let client = &mut world["client"];
    assert_eq!(effects(client, here), [], "its effects stay the server's");
    let played = client.read_triggered::<CueExecuted>();
    assert_eq!(played.len(), 1, "the hit, once");
    assert_eq!(played[0].tag.name(), "cue.hit");
    assert!(same(played[0].magnitude, -20.0));
}

#[test]
fn a_one_shot_a_client_predicted_plays_once() {
    let mut world = net();
    let (_, here) = mage(&mut world, true);
    world["client"].watch_triggered::<CueExecuted>();
    world["client"]
        .world_mut()
        .activate_ability(here, Fireball)
        .unwrap();
    world.ticks(30);
    let played: Vec<_> = world["client"]
        .read_triggered::<CueExecuted>()
        .into_iter()
        .filter(|executed| executed.tag.name() == "cue.hit")
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
        .apply_effect(there, FireballCooldown);
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
        .grant_ability(there, Fireball)
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
        .apply_effect(there, FireballCooldown);
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
        .activate_ability(here, Fireball)
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
        .add("state.stunned");
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
        .activate_ability(here, Fireball)
        .unwrap()
        .ability();
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
        .activate_ability(here, Fireball)
        .unwrap()
        .ability();
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

/// Every content mismatch a client was told of.
#[derive(Resource, Default)]
struct Mismatches(Vec<ContentMismatch>);

fn watch_mismatches(app: &mut TestApp) {
    app.world_mut().init_resource::<Mismatches>();
    app.world_mut().add_observer(
        |mismatch: On<ContentMismatch>, mut seen: ResMut<Mismatches>| {
            seen.0.push(*mismatch);
        },
    );
}

/// Rechecks `world`'s effects with `effects` added, as a reload does: the
/// numbers effects had kept, new ones after.
fn reload_with(world: &mut World, effects: &[EffectData]) {
    let mut all = vec![
        EffectData::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))
            .cue("cue.hit"),
        EffectData::lasting("fireball.cooldown", 1.0)
            .grant("cooldown.fireball")
            .cue("cue.cooling"),
    ];
    all.extend_from_slice(effects);
    let reloaded = world
        .resource::<EffectRegistry>()
        .recheck(
            &all,
            EffectChecks {
                tags: world.resource(),
                curves: &CurveRegistry::default(),
                attributes: world.resource(),
                executions: world.resource(),
                calculations: world.resource(),
                routes: world.resource(),
            },
        )
        .unwrap();
    world.insert_resource(reloaded);
}

#[test]
fn a_client_numbering_content_as_its_server_does_is_told_nothing() {
    let mut world = net();
    watch_mismatches(&mut world["client"]);
    world.ticks(20);
    assert_eq!(world["client"].resource::<Mismatches>().0, []);
    assert_eq!(
        ContentFingerprint::of(world["client"].world()),
        ContentFingerprint::of(world["server"].world())
    );
}

#[test]
fn a_server_reloading_alone_tells_its_clients_they_number_content_otherwise() {
    let mut world = net();
    watch_mismatches(&mut world["client"]);
    world.ticks(20);
    // `aura` sorts first, but a reload numbers it after what's there.
    reload_with(
        world["server"].world_mut(),
        &[EffectData::lasting("aura", 1.0)],
    );
    world.ticks(20);
    let server = ContentFingerprint::of(world["server"].world());
    let seen = world["client"].resource::<Mismatches>().0.clone();
    assert_eq!(seen.len(), 1, "told once: {seen:?}");
    assert_eq!(seen[0].server, server);
    assert_ne!(seen[0].client, server);

    // Reloaded alike, the client numbers it as its server does.
    reload_with(
        world["client"].world_mut(),
        &[EffectData::lasting("aura", 1.0)],
    );
    world.ticks(20);
    assert_eq!(
        world["client"].resource::<Mismatches>().0.len(),
        1,
        "and isn't told again"
    );
    assert_eq!(ContentFingerprint::of(world["client"].world()), server);
}

#[test]
fn a_client_with_other_content_is_told_as_it_connects() {
    let mut world = net();
    watch_mismatches(&mut world["client"]);
    reload_with(
        world["client"].world_mut(),
        &[EffectData::lasting("aura", 1.0)],
    );
    world.ticks(20);
    assert_ne!(
        world["client"].resource::<Mismatches>().0,
        [],
        "its content isn't its server's"
    );
}

#[test]
fn context_data_added_in_another_order_is_told_as_a_mismatch() {
    let mut world = net();
    watch_mismatches(&mut world["client"]);
    world.ticks(20);
    assert_eq!(world["client"].resource::<Mismatches>().0, [], "alike");
    // The client adds the same types, the other way round.
    let mut reversed = App::new();
    reversed
        .add_context_data::<Wound>()
        .add_context_data::<Impact>();
    let reversed = reversed.world().resource::<ContextDataRegistry>().clone();
    world["client"].insert_resource(reversed);
    world.ticks(20);
    let seen = world["client"].resource::<Mismatches>().0.clone();
    assert_eq!(seen.len(), 1, "told once: {seen:?}");
    assert_ne!(seen[0].client.context_data, seen[0].server.context_data);
    assert_eq!(
        seen[0].client.effects, seen[0].server.effects,
        "the rest alike"
    );
}

/// Spawns, on the server, an entity replicated to the client: it, and what
/// it is on the client once there.
fn replicated(world: &mut TestWorld) -> (Entity, Entity) {
    let there = world["server"].spawn(Replicate::to_clients(NetworkTarget::All));
    world
        .until(|world| mapped(&world["client"], there).is_some())
        .describe("the entity replicates")
        .within_ticks(50);
    (there, mapped(&world["client"], there).unwrap())
}

/// A hit, as the server put it on a context.
fn struck() -> Hit {
    Hit {
        point: Vec3::new(1.0, 2.0, 3.0),
        normal: Vec3::Z,
    }
}

/// `effect` from `source`, struck by `causer`, come from `object`, marking
/// `marked`: the server's entities.
fn carrying(
    effect: impl Into<EffectSpec>,
    source: Entity,
    causer: Entity,
    object: Entity,
    marked: Entity,
) -> EffectSpec {
    effect
        .into()
        .source(source)
        .causer(causer)
        .object(object)
        .hit(struck())
        .origin(Vec3::new(-5.0, 0.0, 0.0))
        .with(Impact {
            marked,
            headshot: true,
        })
        .with(Wound { deep: true })
}

#[test]
fn a_replicated_effects_context_names_the_clients_entities() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let (arrow, arrow_here) = replicated(&mut world);
    let (bow, bow_here) = replicated(&mut world);
    let (marked, marked_here) = replicated(&mut world);
    world["server"]
        .world_mut()
        .apply_effect(there, carrying(FireballCooldown, there, arrow, bow, marked));
    world
        .until(|world| {
            world["client"]
                .entity(here)
                .try_get::<ActiveEffects>()
                .is_some_and(|effects| effects.iter().count() == 1)
        })
        .describe("the cooldown replicates")
        .within_ticks(20);
    let client = &world["client"];
    let shown = client
        .entity(here)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    let replica = &client.entity(shown).get::<EffectReplica>().context;
    assert_eq!(replica.source, Some(here), "the replica, mapped");
    assert_eq!(replica.causer, Some(arrow_here));
    assert_eq!(replica.object, Some(bow_here));
    assert_eq!(replica.data[0].entities, [marked_here]);
    let context = &client.entity(shown).get::<ActiveEffect>().spec().context;
    assert_eq!(context.source, Some(here), "the effect shown, mapped");
    assert_eq!(context.causer, Some(arrow_here));
    assert_eq!(context.object, Some(bow_here));
    assert_eq!(context.hit, Some(struck()));
    assert_eq!(context.origin, Some(Vec3::new(-5.0, 0.0, 0.0)));
    assert_eq!(
        context.get::<Impact>(),
        Some(&Impact {
            marked: marked_here,
            headshot: true,
        })
    );
    assert_eq!(context.get::<Wound>(), Some(&Wound { deep: true }));
}

#[test]
fn a_one_shot_the_server_plays_is_told_its_context_on_the_client() {
    let mut world = net();
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
    ));
    let (archer, archer_here) = replicated(&mut world);
    let (arrow, arrow_here) = replicated(&mut world);
    let (bow, bow_here) = replicated(&mut world);
    let here = mapped(&world["client"], there).expect("the actor replicated");
    world["client"].watch_triggered::<CueExecuted>();
    world["server"]
        .world_mut()
        .apply_effect(there, carrying(FireballCost, archer, arrow, bow, there));
    world.ticks(20);
    let played = world["client"].read_triggered::<CueExecuted>();
    let [played] = &played[..] else {
        panic!("the hit, once: {played:?}");
    };
    assert_eq!(played.tag.name(), "cue.hit");
    let context = &played.context;
    assert_eq!(context.source, Some(archer_here), "mapped");
    assert_eq!(context.causer, Some(arrow_here));
    assert_eq!(context.object, Some(bow_here));
    assert_eq!(context.hit, Some(struck()));
    assert_eq!(context.origin, Some(Vec3::new(-5.0, 0.0, 0.0)));
    assert_eq!(
        context.get::<Impact>(),
        Some(&Impact {
            marked: here,
            headshot: true,
        })
    );
}

/// The bit a server's own activations have set.
const SERVERS: u64 = 1 << 63;

/// The bit an activation derived from another's has set.
const DERIVED: u64 = 1 << 62;

/// An actor on the server, controlled by the client, given what `grant`
/// grants it: it, and its actor on the client, once every ability it has
/// replicated.
fn granted(world: &mut TestWorld, grant: impl FnOnce(&mut World, Entity)) -> (Entity, Entity) {
    let link = link(world);
    let there = world["server"].spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: link,
            lifetime: Lifetime::default(),
        },
    ));
    grant(world["server"].world_mut(), there);
    let count = world["server"]
        .entity(there)
        .get::<Abilities>()
        .iter()
        .count();
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
                            .filter(|ability| client.entity(*ability).has::<GrantedAbility>())
                            .count()
                            == count
                    })
            })
        })
        .describe("the actor and its abilities replicate")
        .within_ticks(50);
    (there, mapped(&world["client"], there).unwrap())
}

/// `actor`'s granted `A`, as `app` has it.
fn ability<A: Ability>(app: &TestApp, actor: Entity) -> Entity {
    let id = app.resource::<AbilityRegistry>().id_of::<A>().unwrap();
    app.entity(actor)
        .get::<Abilities>()
        .iter()
        .find(|ability| {
            app.entity(*ability)
                .try_get::<GrantedAbility>()
                .is_some_and(|granted| granted.def().id == id)
        })
        .unwrap()
}

/// The activation `actor`'s `A` runs under in `app`, if it's active.
fn running<A: Ability>(app: &TestApp, actor: Entity) -> Option<u64> {
    app.entity(ability::<A>(app, actor))
        .try_get::<ActiveAbility>()
        .map(|active| active.activation)
}

#[test]
fn a_server_only_ability_runs_on_the_server_alone() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Smite);
    });
    let activated = world["client"]
        .world_mut()
        .activate_ability(here, Smite)
        .unwrap();
    assert!(
        matches!(activated, Activated::Asked(_)),
        "asked of the server, not run here: {activated:?}"
    );
    for _ in 0..20 {
        world.tick();
        assert_eq!(
            running::<Smite>(&world["client"], here),
            None,
            "never run here"
        );
        assert_eq!(world["client"].count::<With<Smiting>>(), 0);
    }
    let server = &world["server"];
    assert!(running::<Smite>(server, there).is_some(), "run there");
    server
        .entity(ability::<Smite>(server, there))
        .assert_has::<Smiting>();
    assert!(same(mana(&world["server"], there), 10.0), "paid there");
    assert!(
        same(mana(&world["client"], here), 10.0),
        "and shown here, replicated"
    );
}

#[test]
fn a_server_only_ability_the_server_cant_pay_for_fails_on_the_client() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Smite);
    });
    world["server"]
        .entity_mut(there)
        .get_mut::<Base<Mana>>()
        .unwrap()
        .set(10.0);
    world["client"].watch_triggered::<AbilityFailed>();
    let activated = world["client"]
        .world_mut()
        .activate_ability(here, Smite)
        .unwrap();
    assert!(
        matches!(activated, Activated::Asked(_)),
        "asked of the server, not run here: {activated:?}"
    );
    world.ticks(20);
    assert_eq!(running::<Smite>(&world["server"], there), None);
    let smite = world["client"]
        .resource::<AbilityRegistry>()
        .id_of::<Smite>()
        .unwrap();
    world["client"].assert_triggered::<AbilityFailed>(|failed| {
        failed.entity == here && failed.id == smite && failed.reason == Failure::Cost
    });
}

#[test]
fn a_server_initiated_ability_a_client_asks_for_mirrors_the_servers() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Ward);
    });
    let activated = world["client"]
        .world_mut()
        .activate_ability(here, Ward)
        .unwrap();
    assert!(
        matches!(activated, Activated::Asked(_)),
        "asked of the server, not run here: {activated:?}"
    );
    assert_eq!(
        running::<Ward>(&world["client"], here),
        None,
        "asked, not run yet"
    );
    world
        .until(|world| running::<Ward>(&world["client"], here).is_some())
        .describe("the client runs what the server started")
        .within_ticks(20);
    let server = &world["server"];
    let activation = running::<Ward>(server, there).expect("the server runs it");
    assert_eq!(
        running::<Ward>(&world["client"], here),
        Some(activation),
        "under the server's activation"
    );
    let client = &world["client"];
    client
        .entity(ability::<Ward>(client, here))
        .assert_has::<Warding>();
    for _ in 0..20 {
        world.tick();
        let mana = mana(&world["client"], here);
        assert!(
            same(mana, 10.0) || same(mana, 30.0),
            "never paid twice: {mana}"
        );
    }
    assert!(same(mana(&world["server"], there), 10.0));
    assert!(
        same(mana(&world["client"], here), 10.0),
        "as the server says"
    );
    assert_eq!(running::<Ward>(&world["client"], here), Some(activation));
}

#[test]
fn a_server_initiated_ability_the_server_ends_ends_on_the_client() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Ward);
    });
    let activated = world["client"]
        .world_mut()
        .activate_ability(here, Ward)
        .unwrap();
    assert!(
        matches!(activated, Activated::Asked(_)),
        "asked of the server, not run here: {activated:?}"
    );
    world
        .until(|world| running::<Ward>(&world["client"], here).is_some())
        .describe("the client runs what the server started")
        .within_ticks(20);
    let ward = ability::<Ward>(&world["server"], there);
    world["server"].world_mut().end_ability(ward);
    world
        .until(|world| running::<Ward>(&world["client"], here).is_none())
        .describe("the client's ends with the server's")
        .within_ticks(20);
    assert_eq!(world["client"].count::<With<Warding>>(), 0);
}

#[test]
fn a_server_initiated_ability_the_server_triggers_runs_on_the_client() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Ward);
    });
    AbilityWorld::send_event(
        world["server"].world_mut(),
        GameplayEvent::new(there, TagKey::named("event.ward")),
    );
    let activation = running::<Ward>(&world["server"], there).expect("triggered");
    assert_ne!(activation & SERVERS, 0, "the server's own");
    world
        .until(|world| running::<Ward>(&world["client"], here) == Some(activation))
        .describe("the client runs what the server triggered")
        .within_ticks(20);
    let client = &world["client"];
    client
        .entity(ability::<Ward>(client, here))
        .assert_has::<Warding>();
    world.ticks(20);
    assert!(same(mana(&world["client"], here), 10.0), "paid once");
}

#[test]
fn a_server_initiated_ability_the_client_cancels_is_cancelled_on_the_server() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Ward);
    });
    AbilityWorld::send_event(
        world["server"].world_mut(),
        GameplayEvent::new(there, TagKey::named("event.ward")),
    );
    world
        .until(|world| running::<Ward>(&world["client"], here).is_some())
        .describe("the client runs what the server triggered")
        .within_ticks(20);
    let ward = ability::<Ward>(&world["client"], here);
    world["client"].world_mut().cancel_ability(ward);
    assert_eq!(running::<Ward>(&world["client"], here), None);
    world
        .until(|world| running::<Ward>(&world["server"], there).is_none())
        .describe("the server's is cancelled")
        .within_ticks(20);
    world.ticks(10);
    assert_eq!(
        running::<Ward>(&world["client"], here),
        None,
        "and not started again"
    );
}

#[test]
fn a_local_only_ability_runs_on_the_client_alone() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Zoom);
    });
    let activated = world["client"]
        .world_mut()
        .activate_ability(here, Zoom)
        .unwrap();
    assert!(
        matches!(activated, Activated::Running(_)),
        "running here: {activated:?}"
    );
    let client = &world["client"];
    assert!(running::<Zoom>(client, here).is_some(), "run at once");
    client
        .entity(ability::<Zoom>(client, here))
        .assert_has::<Zooming>();
    for _ in 0..20 {
        world.tick();
        assert_eq!(running::<Zoom>(&world["server"], there), None, "never sent");
    }
    assert!(
        running::<Zoom>(&world["client"], here).is_some(),
        "still running"
    );
}

#[test]
fn a_client_asking_for_a_local_only_ability_is_rejected() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Zoom);
    });
    world["client"].watch_triggered::<AbilityFailed>();
    let zoom = world["client"]
        .resource::<AbilityRegistry>()
        .id_of::<Zoom>()
        .unwrap();
    world["client"].world_mut().write_message(AbilityRequest {
        actor: here,
        ability: zoom,
        activation: 1,
    });
    world.ticks(20);
    assert_eq!(running::<Zoom>(&world["server"], there), None);
    world["client"].assert_triggered::<AbilityFailed>(|failed| {
        failed.id == zoom && failed.reason == Failure::NotGranted
    });
}

#[test]
fn a_predicted_trigger_runs_at_once_under_the_activation_the_server_runs_it_under() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Flare);
        server.grant_ability(there, Spark);
    });
    world["client"]
        .world_mut()
        .activate_ability(here, Flare)
        .unwrap();
    let client = &world["client"];
    let flare = running::<Flare>(client, here).expect("predicted");
    let spark = running::<Spark>(client, here).expect("triggered, predicted at once");
    assert_ne!(spark, flare);
    assert_ne!(spark & DERIVED, 0, "derived");
    assert_eq!(spark & SERVERS, 0, "from the client's");
    client
        .entity(ability::<Spark>(client, here))
        .assert_has::<Sparking>();
    world
        .until(|world| running::<Spark>(&world["server"], there).is_some())
        .describe("the server sparks too")
        .within_ticks(20);
    let server = &world["server"];
    assert_eq!(running::<Flare>(server, there), Some(flare));
    assert_eq!(
        running::<Spark>(server, there),
        Some(spark),
        "under the same activation"
    );
    world.ticks(20);
    let client = &world["client"];
    assert_eq!(running::<Flare>(client, here), Some(flare));
    assert_eq!(
        running::<Spark>(client, here),
        Some(spark),
        "confirmed, kept"
    );
    assert!(same(mana(client, here), 10.0), "the flare paid once");
}

#[test]
fn a_predicted_trigger_is_undone_with_its_rejected_parent() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Flare);
        server.grant_ability(there, Spark);
    });
    world["server"]
        .entity_mut(there)
        .get_mut::<OwnedTags>()
        .unwrap()
        .add("state.stunned");
    world["client"]
        .world_mut()
        .activate_ability(here, Flare)
        .unwrap();
    assert!(
        running::<Spark>(&world["client"], here).is_some(),
        "predicted"
    );
    world
        .until(|world| {
            let client = &world["client"];
            running::<Flare>(client, here).is_none() && running::<Spark>(client, here).is_none()
        })
        .describe("the flare and its spark are undone")
        .within_ticks(20);
    world.ticks(10);
    let server = &world["server"];
    assert_eq!(running::<Flare>(server, there), None, "refused");
    assert_eq!(running::<Spark>(server, there), None, "never triggered");
    assert_eq!(world["client"].count::<With<Sparking>>(), 0);
    assert!(same(mana(&world["client"], here), 30.0), "mana back");
}

#[test]
fn abilities_a_client_activates_by_tag_are_predicted_and_sent() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let client = world["client"].world_mut();
    client
        .commands()
        .entity(here)
        .activate_tagged([TagKey::named("ability")].into_iter().collect());
    client.flush();
    assert!(casting(&world["client"], here), "predicted at once");
    world.tick();
    assert!(same(mana(&world["client"], here), 10.0), "paid, predicted");
    world
        .until(|world| casting(&world["server"], there))
        .describe("the server casts too")
        .within_ticks(20);
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 10.0));
    assert!(same(mana(&world["client"], here), 10.0), "paid once");
}

#[test]
fn a_granted_ability_a_client_activates_is_predicted_and_sent() {
    let mut world = net();
    let (there, here) = mage(&mut world, true);
    let fireball = ability::<Fireball>(&world["client"], here);
    let activated = AbilityWorld::activate(world["client"].world_mut(), fireball).unwrap();
    assert_eq!(activated, Activated::Running(fireball), "running here");
    assert!(casting(&world["client"], here), "predicted at once");
    world
        .until(|world| casting(&world["server"], there))
        .describe("the server casts too")
        .within_ticks(20);
    world.ticks(20);
    assert!(same(mana(&world["server"], there), 10.0));
    assert!(same(mana(&world["client"], here), 10.0), "paid once");
}

#[test]
fn a_granted_server_only_ability_a_client_activates_is_asked_for() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Smite);
    });
    let smite = ability::<Smite>(&world["client"], here);
    let activated = AbilityWorld::activate(world["client"].world_mut(), smite).unwrap();
    assert_eq!(activated, Activated::Asked(smite), "asked, not run here");
    assert_eq!(running::<Smite>(&world["client"], here), None);
    world
        .until(|world| running::<Smite>(&world["server"], there).is_some())
        .describe("the server runs it")
        .within_ticks(20);
    assert_eq!(running::<Smite>(&world["client"], here), None, "never here");
}

#[test]
fn a_server_initiated_ability_started_and_ended_at_once_isnt_left_running_on_the_client() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Ward);
    });
    world["client"].watch_triggered::<AbilityActivated>();
    // Started and ended in one server frame: its start and its end go out
    // together, the start first.
    AbilityWorld::send_event(
        world["server"].world_mut(),
        GameplayEvent::new(there, TagKey::named("event.ward")),
    );
    let activation = running::<Ward>(&world["server"], there).expect("triggered");
    let ward = ability::<Ward>(&world["server"], there);
    world["server"].world_mut().end_ability(ward);
    assert_eq!(running::<Ward>(&world["server"], there), None, "ended");
    world.ticks(20);
    let id = world["client"]
        .resource::<AbilityRegistry>()
        .id_of::<Ward>()
        .unwrap();
    world["client"].assert_triggered::<AbilityActivated>(|activated| {
        activated.entity == here && activated.id == id && activated.activation == activation
    });
    assert_eq!(
        running::<Ward>(&world["client"], here),
        None,
        "started, then ended"
    );
    assert_eq!(world["client"].count::<With<Warding>>(), 0);
}

#[test]
fn a_predicted_trigger_under_the_servers_activation_runs_on_the_server_alone() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Beacon);
        server.grant_ability(there, Spark);
    });
    AbilityWorld::send_event(
        world["server"].world_mut(),
        GameplayEvent::new(there, TagKey::named("event.beacon")),
    );
    let server = &world["server"];
    let beacon = running::<Beacon>(server, there).expect("triggered");
    assert_ne!(beacon & SERVERS, 0, "the server's own");
    let spark = running::<Spark>(server, there).expect("sparked under the beacon");
    assert_ne!(spark & SERVERS, 0, "the server's, derived");
    assert_ne!(spark & DERIVED, 0, "derived");
    world
        .until(|world| running::<Beacon>(&world["client"], here) == Some(beacon))
        .describe("the client mirrors the beacon")
        .within_ticks(20);
    let client = &world["client"];
    client
        .entity(ability::<Beacon>(client, here))
        .assert_has::<Flaring>();
    for _ in 0..20 {
        assert_eq!(
            running::<Spark>(&world["client"], here),
            None,
            "not predicted under the server's activation"
        );
        assert_eq!(world["client"].count::<With<Sparking>>(), 0);
        world.tick();
    }
    assert_eq!(
        running::<Spark>(&world["server"], there),
        Some(spark),
        "the server runs it"
    );
    assert_eq!(running::<Beacon>(&world["client"], here), Some(beacon));
}

#[test]
fn a_local_only_ability_an_event_on_the_client_triggers_runs_there_alone() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Lens);
    });
    AbilityWorld::send_event(
        world["client"].world_mut(),
        GameplayEvent::new(here, TagKey::named("event.lens")),
    );
    let client = &world["client"];
    assert!(running::<Lens>(client, here).is_some(), "run at once");
    client
        .entity(ability::<Lens>(client, here))
        .assert_has::<Zooming>();
    for _ in 0..20 {
        world.tick();
        assert_eq!(running::<Lens>(&world["server"], there), None, "never sent");
    }
    assert!(
        running::<Lens>(&world["client"], here).is_some(),
        "still running"
    );
}

#[test]
fn a_local_only_ability_a_tag_on_the_client_triggers_runs_there_alone() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Scope);
    });
    world["client"]
        .entity_mut(here)
        .get_mut::<OwnedTags>()
        .unwrap()
        .add("state.aiming");
    world
        .until(|world| running::<Scope>(&world["client"], here).is_some())
        .describe("the client runs it as the tag comes")
        .within_ticks(5);
    let client = &world["client"];
    client
        .entity(ability::<Scope>(client, here))
        .assert_has::<Zooming>();
    for _ in 0..20 {
        world.tick();
        assert_eq!(
            running::<Scope>(&world["server"], there),
            None,
            "never sent"
        );
    }
    assert!(
        running::<Scope>(&world["client"], here).is_some(),
        "still running"
    );
}

#[test]
fn a_client_cant_cancel_a_server_only_ability() {
    let mut world = net();
    let (there, here) = granted(&mut world, |server, there| {
        server.grant_ability(there, Smite);
        server.grant_ability(there, Flare);
    });
    // Enough for both.
    world["server"]
        .entity_mut(there)
        .get_mut::<Base<Mana>>()
        .unwrap()
        .set(40.0);
    world
        .until(|world| same(mana(&world["client"], here), 40.0))
        .describe("the client sees its mana")
        .within_ticks(20);
    world["client"]
        .world_mut()
        .activate_ability(here, Smite)
        .unwrap();
    let flare = world["client"]
        .world_mut()
        .activate_ability(here, Flare)
        .unwrap()
        .ability();
    world
        .until(|world| {
            let server = &world["server"];
            running::<Smite>(server, there).is_some() && running::<Flare>(server, there).is_some()
        })
        .describe("the server runs both")
        .within_ticks(20);
    let activation = running::<Smite>(&world["server"], there).unwrap();
    world["client"]
        .world_mut()
        .write_message(AbilityCancelRequest {
            actor: here,
            activation,
        });
    // A cancel the server heeds, sent after: as it's heeded, the other's
    // been heard.
    world["client"].world_mut().cancel_ability(flare);
    world
        .until(|world| running::<Flare>(&world["server"], there).is_none())
        .describe("the server cancels the flare")
        .within_ticks(20);
    for _ in 0..20 {
        world.tick();
        assert_eq!(
            running::<Smite>(&world["server"], there),
            Some(activation),
            "ignored: not the client's to cancel"
        );
    }
    let server = &world["server"];
    server
        .entity(ability::<Smite>(server, there))
        .assert_has::<Smiting>();
}
