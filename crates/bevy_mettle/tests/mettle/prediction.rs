//! Prediction: a client activates its own ability at once, the server decides,
//! and the client keeps or undoes what it predicted. The transport is by hand
//! here: what an adapter (lightyear's) ships.

use bevy::ecs::entity::{EntityHashMap, MapEntities};
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
            Effect::instant("focus.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Additive, -0.5)),
            Effect::lasting("burn", 5.0)
                .period(0.5, false)
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -1.0)),
            Effect::lasting("ward", 5.0)
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, 1.0))
                .stacking(Stacking {
                    by: StackBy::Target,
                    limit: 3,
                    reapply: Reapply::Refresh,
                    reset_period: false,
                    expire: Expire::ClearAll,
                    overflow: Vec::new(),
                }),
            Effect::infinite("blessed").modifier(
                EffectModifier::new::<Mana>(Operation::Flat, 10.0)
                    .condition(Condition::source_has("state.stunned")),
            ),
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
    let charge = Ability::new("charge", "cast")
        .cost("fireball.cost")
        .commit(Commit::Manual);
    let focus = Ability::new("focus", "cast").cost("focus.cost");
    let abilities = AbilityRegistry::build(
        &[fireball, charge, focus],
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
        Self::with(&["fireball"], &[])
    }

    /// The mage granted `abilities`, with `modifiers` of its own.
    fn with(abilities: &[&'static str], modifiers: &[Modifier]) -> Self {
        let mut server = world(NetRole::Server);
        let mut client = world(NetRole::Client);
        let there = actor(&mut server, modifiers.to_vec());
        let here = actor(&mut client, modifiers.to_vec());
        for id in abilities {
            let ability = server.world_mut().grant_ability(there, *id, 1.0).unwrap();
            server.tick();
            let replica = server
                .world()
                .get::<AbilityReplica>(ability)
                .unwrap()
                .clone();
            client.spawn((replica, AbilityOf(here)));
        }
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

    /// The mage's granted `id`, on the client.
    fn ability_here(&self, id: &str) -> Entity {
        self.client
            .world()
            .get::<Abilities>(self.here)
            .unwrap()
            .iter()
            .find(|ability| {
                self.client
                    .world()
                    .get::<GrantedAbility>(*ability)
                    .is_some_and(|granted| granted.id().as_str() == id)
            })
            .unwrap()
    }

    /// The mage's granted `id`, on the server.
    fn ability_there(&self, id: &str) -> Entity {
        self.server
            .world()
            .get::<Abilities>(self.there)
            .unwrap()
            .iter()
            .find(|ability| {
                self.server
                    .world()
                    .get::<GrantedAbility>(*ability)
                    .is_some_and(|granted| granted.id().as_str() == id)
            })
            .unwrap()
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

#[test]
fn a_commit_after_the_answer_predicts_nothing_more() {
    let mut game = Game::with(&["charge"], &[]);
    game.client
        .world_mut()
        .activate_ability(game.here, "charge")
        .unwrap();
    game.client.tick();
    game.ask();
    game.answer();
    let here = game.ability_here("charge");
    assert!(game.client.world_mut().commit_ability(here));
    game.client.tick();
    let there = game.ability_there("charge");
    assert!(game.server.world_mut().commit_ability(there));
    game.server.tick();
    game.answer();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "the server's, paid once"
    );
}

#[test]
fn a_predicted_periodic_effect_predicts_nothing_once_answered() {
    let mut game = Game::new();
    game.client
        .world_mut()
        .apply_effect(game.here, EffectSpec::new("burn").activation(7));
    game.client.secs(1.0);
    assert!(
        get::<Mana>(&game.client, game.here) < 30.0,
        "burning, predicted"
    );
    game.client.world_mut().confirm_prediction(7);
    game.client.tick();
    game.client.secs(1.0);
    assert!(
        close(get::<Mana>(&game.client, game.here), 30.0),
        "the server's mana, nothing left over"
    );
}

/// The server's `effect` on its mage, shown on the client.
fn shown(game: &mut Game, effect: &'static str) -> Entity {
    game.server
        .world_mut()
        .apply_effect(game.there, EffectSpec::new(effect));
    game.server.tick();
    game.answer();
    game.client
        .world()
        .get::<ActiveEffects>(game.here)
        .unwrap()
        .iter()
        .find(|shown| game.client.world().get::<Proxy>(*shown).is_some())
        .unwrap()
}

#[test]
fn a_stack_predicted_onto_the_servers_effect_is_kept_apart() {
    let mut game = Game::new();
    let ward = shown(&mut game, "ward");
    game.client
        .world_mut()
        .apply_effect(game.here, EffectSpec::new("ward").activation(7));
    game.client.tick();
    let stacks = |game: &Game| {
        game.client
            .world()
            .get::<ActiveEffect>(ward)
            .map(ActiveEffect::stacks)
    };
    assert_eq!(stacks(&game), Some(2));
    // The server's replica restated: its own stack, the time refreshed.
    game.client
        .world_mut()
        .get_mut::<EffectReplica>(ward)
        .unwrap()
        .remaining = Some(5.0);
    game.client.tick();
    assert_eq!(stacks(&game), Some(2), "the predicted stack on top");
    game.client.world_mut().reject_prediction(7);
    game.client.tick();
    assert_eq!(stacks(&game), Some(1), "the server's effect stays");
}

#[test]
fn a_shown_effect_counts_its_source_as_it_was() {
    let mut game = Game::new();
    let server_caster = actor(&mut game.server, []);
    let client_caster = actor(&mut game.client, []);
    own(&mut game.server, server_caster, "state.stunned", true);
    game.server.tick();
    game.server
        .world_mut()
        .apply_effect(game.there, EffectSpec::new("blessed").source(server_caster));
    game.server.tick();
    let effect = game
        .server
        .world()
        .get::<ActiveEffects>(game.there)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    let mut replica = game
        .server
        .world()
        .get::<EffectReplica>(effect)
        .unwrap()
        .clone();
    let mut mapping = EntityHashMap::default();
    mapping.insert(server_caster, client_caster);
    replica.map_entities(&mut mapping);
    assert_eq!(replica.spec.source, Some(client_caster), "mapped");
    game.client.spawn((replica, EffectOf(game.here)));
    game.client.ticks(2);
    assert!(
        close(get::<Mana>(&game.client, game.here), 40.0),
        "as the source owned as it applied"
    );
}

#[test]
fn a_predicted_cost_comes_to_what_the_server_works_out() {
    let mut game = Game::with(&["focus"], &[Modifier::additive::<Mana>(0.5)]);
    game.client
        .world_mut()
        .activate_ability(game.here, "focus")
        .unwrap();
    game.client.tick();
    game.ask();
    let server = get::<Mana>(&game.server, game.there);
    assert!(close(server, 22.5), "half the base, then +50%: {server}");
    let predicted = get::<Mana>(&game.client, game.here);
    assert!(close(predicted, server), "predicted {predicted}");
}

#[test]
fn another_sources_effect_under_the_same_activation_isnt_taken_for_mine() {
    let mut game = Game::new();
    game.client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.tick();
    let activation = game
        .client
        .world()
        .get::<ActiveAbility>(game.ability_here("fireball"))
        .unwrap()
        .activation;
    let elsewhere = game.client.spawn(());
    game.client.spawn((
        EffectReplica {
            spec: EffectSpec::new("fireball.cooldown")
                .source(elsewhere)
                .activation(activation),
            stacks: 1,
            duration: Some(1.0),
            remaining: Some(1.0),
            inhibited: false,
            source_tags: Tags::new(),
            captured: Vec::new(),
        },
        EffectOf(game.here),
    ));
    game.client.tick();
    assert_eq!(
        game.cooldowns_here(),
        [(true, false), (false, true)],
        "another client's, from its own mage"
    );
}

#[test]
fn a_request_under_the_servers_bit_is_rejected() {
    let mut game = Game::new();
    game.server.world_mut().write_message(AbilityRequest {
        actor: game.there,
        ability: Id::from_static("fireball"),
        activation: 1 << 63 | 5,
    });
    game.server.tick();
    let replies: Vec<AbilityReply> = game
        .server
        .world_mut()
        .resource_mut::<Messages<AbilityReply>>()
        .drain()
        .collect();
    assert_eq!(replies.len(), 1);
    assert_eq!(replies[0].rejected, Some(Failure::NotGranted));
    assert!(close(get::<Mana>(&game.server, game.there), 30.0));
}

#[test]
fn an_abilitys_replica_restated_changes_its_level() {
    let mut game = Game::new();
    let ability = game.ability_here("fireball");
    game.client
        .world_mut()
        .get_mut::<AbilityReplica>(ability)
        .unwrap()
        .level = 3.0;
    game.client.tick();
    assert_eq!(
        game.client
            .world()
            .get::<GrantedAbility>(ability)
            .unwrap()
            .level(),
        3.0
    );
}
