//! Prediction: a client activates its own ability at once, the server decides,
//! and the client keeps or undoes what it predicted. The transport is by hand
//! here: what an adapter (lightyear's) ships.

use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Mana, actor, attributes, close, get, own};

#[derive(Component, Default)]
struct Cast;

const TAGS: &[&str] = &[
    "damage",
    "state",
    "state.stunned",
    "cooldown",
    "cooldown.fireball",
];

fn tags() -> TagTree {
    let defs: Vec<Tag> = TAGS
        .iter()
        .map(|&id| Tag {
            id: Id::from_static(id),
        })
        .collect();
    TagTree::build(&defs).unwrap()
}

/// A world playing as `role`, with fireball.
fn world(role: NetRole) -> TestApp {
    let mut app = TestApp::builder()
        .with(attributes)
        .with(move |app| {
            app.insert_resource(role);
        })
        .plugins((EffectPlugin::default(), AbilityPlugin))
        .with(|app| {
            app.add_behaviour::<Cast>("cast");
        })
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::build(
        &[
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            Effect::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
        ],
        &tags(),
        &CurveTable::default(),
        &attributes,
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
        &tags(),
        &effects,
        world.resource::<Behaviours>(),
    )
    .unwrap();
    world.insert_resource(effects);
    world.insert_resource(abilities);
    app
}

/// A server and a client, the same mage in each, granted fireball by the
/// server: its replica shipped to the client.
struct Game {
    server: TestApp,
    client: TestApp,
    /// The mage, on the server.
    there: Entity,
    /// The mage, on the client.
    here: Entity,
}

impl Game {
    fn new() -> Self {
        let mut server = world(NetRole::Server);
        let mut client = world(NetRole::Client);
        let there = actor(&mut server, []);
        let here = actor(&mut client, []);
        let ability = server
            .world_mut()
            .grant_ability(there, "fireball", 1.0)
            .unwrap();
        server.tick();
        let replica = server
            .world()
            .get::<AbilityReplica>(ability)
            .unwrap()
            .clone();
        client.spawn((replica, AbilityOf(here)));
        client.tick();
        Self {
            server,
            client,
            there,
            here,
        }
    }

    /// Ships the client's requests to the server, and runs it.
    fn ask(&mut self) {
        let requests: Vec<AbilityRequest> = self
            .client
            .world_mut()
            .resource_mut::<Messages<AbilityRequest>>()
            .drain()
            .collect();
        for request in requests {
            self.server.world_mut().write_message(AbilityRequest {
                actor: self.there,
                ..request
            });
        }
        self.server.tick();
    }

    /// Ships the server's replies and the mage's state (its mana, its effects'
    /// replicas) to the client, and runs it.
    fn answer(&mut self) {
        let replies: Vec<AbilityReply> = self
            .server
            .world_mut()
            .resource_mut::<Messages<AbilityReply>>()
            .drain()
            .collect();
        for reply in replies {
            self.client.world_mut().write_message(AbilityReply {
                actor: self.here,
                ..reply
            });
        }
        let mana = self
            .server
            .world()
            .get::<Base<Mana>>(self.there)
            .unwrap()
            .get();
        self.client
            .world_mut()
            .get_mut::<Base<Mana>>(self.here)
            .unwrap()
            .set(mana);
        let effects: Vec<EffectReplica> = self
            .server
            .world()
            .get::<ActiveEffects>(self.there)
            .map(|effects| effects.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|effect| self.server.world().get::<EffectReplica>(effect).cloned())
            .collect();
        for replica in effects {
            self.client.spawn((replica, EffectOf(self.here)));
        }
        self.client.tick();
    }

    fn cooldowns_here(&self) -> Vec<(bool, bool)> {
        self.client
            .world()
            .get::<ActiveEffects>(self.here)
            .map(|effects| effects.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|effect| {
                (
                    self.client.world().get::<PredictedUnder>(effect).is_some(),
                    self.client.world().get::<Proxy>(effect).is_some(),
                )
            })
            .collect()
    }
}

fn owns(app: &TestApp, actor: Entity, tag: &'static str) -> bool {
    app.world()
        .get::<OwnedTags>(actor)
        .unwrap()
        .contains(&Id::from_static(tag))
}

#[test]
fn a_client_predicts_at_once_and_the_servers_own_replaces_it() {
    let mut game = Game::new();
    game.client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.tick();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "paid, predicted"
    );
    assert_eq!(
        game.cooldowns_here(),
        [(true, false)],
        "a predicted cooldown"
    );
    assert!(owns(&game.client, game.here, "cooldown.fireball"));
    game.ask();
    assert!(
        close(get::<Mana>(&game.server, game.there), 10.0),
        "the server agrees"
    );
    game.answer();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "paid once"
    );
    assert_eq!(
        game.cooldowns_here(),
        [(false, true)],
        "the server's, shown"
    );
    assert!(owns(&game.client, game.here, "cooldown.fireball"));
}

#[test]
fn a_rejected_prediction_is_undone() {
    let mut game = Game::new();
    // The server knows something the client doesn't yet.
    own(&mut game.server, game.there, "state.stunned", true);
    game.client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.tick();
    assert!(close(get::<Mana>(&game.client, game.here), 10.0));
    game.ask();
    assert!(
        close(get::<Mana>(&game.server, game.there), 30.0),
        "refused"
    );
    game.answer();
    assert!(
        close(get::<Mana>(&game.client, game.here), 30.0),
        "mana back"
    );
    assert_eq!(game.cooldowns_here(), [], "the cooldown taken off");
    assert!(!owns(&game.client, game.here, "cooldown.fireball"));
    let ability = game
        .client
        .world()
        .get::<Abilities>(game.here)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    assert!(
        game.client.world().get::<ActiveAbility>(ability).is_none(),
        "ended"
    );
}

#[test]
fn a_shown_effect_takes_its_tags_with_it_as_it_goes() {
    let mut game = Game::new();
    game.client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.tick();
    game.ask();
    game.answer();
    let shown = game
        .client
        .world()
        .get::<ActiveEffects>(game.here)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    game.client.secs(2.0);
    assert!(
        owns(&game.client, game.here, "cooldown.fireball"),
        "not ticked here"
    );
    game.client.world_mut().despawn(shown);
    game.client.tick();
    assert!(
        !owns(&game.client, game.here, "cooldown.fireball"),
        "gone with it"
    );
}
