//! Prediction: a client activates its own ability at once, the server decides,
//! and the client keeps or undoes what it predicted. The transport is by hand
//! here: what an adapter (lightyear's) ships.

use bevy::ecs::entity::{EntityHashMap, MapEntities};
use bevy::ecs::message::Messages;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Mana, Strength, actor, attributes, close, get, own};

#[derive(Component, Default, Behaviour)]
struct Cast;

/// A price paid in mana, and paid back in strength to whoever paid it.
#[derive(MetaAttribute)]
struct Price;

fn route_price(_: &EffectContext, price: f32) -> Vec<BaseChange> {
    vec![
        BaseChange::new::<Mana>(Operation::Flat, -price),
        BaseChange::new::<Strength>(Operation::Flat, 1.0).on_source(),
    ]
}

/// Where a shot is aimed: how far along.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Aim(f32);

/// A shot being fired: aimed.
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct Shoot;

/// How far a shot reaches, as this world knows: the server may know better.
#[derive(Resource)]
struct Reach(f32);

fn in_reach(In(check): In<InputCheck<Aim>>, reach: Res<Reach>) -> bool {
    check.input.0.abs() <= reach.0
}

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
        .with(move |app| {
            // The server knows a shot reaches only 10; the client, 100.
            let reach = if role == NetRole::Server { 10.0 } else { 100.0 };
            app.add_behaviour::<Cast>()
                .add_behaviour::<Shoot>()
                .check_input::<Aim, _>(in_reach)
                .insert_resource(Reach(reach));
        })
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::check(
        &[
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            Effect::instant("bolt.cost").modifier(EffectModifier::meta::<Price>(20.0)),
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
        EffectChecks {
            tags: &tags(),
            curves: &CurveTable::default(),
            attributes: &attributes,
            code: &EffectCode::new().meta_attribute::<Price>(route_price),
        },
    )
    .unwrap();
    let fireball = Ability::new("fireball", "cast")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .blocked_by("state.stunned")
        .cancel_if(Condition::owner_has("state.stunned"));
    let charge = Ability::new("charge", "cast")
        .cost("fireball.cost")
        .commit(Commit::Manual);
    let focus = Ability::new("focus", "cast").cost("focus.cost");
    let shot = Ability::new("shot", "shoot").cost("fireball.cost");
    let bolt = Ability::new("bolt", "cast")
        .cost("bolt.cost")
        .blocked_by("state.stunned");
    let abilities = AbilityRegistry::check(
        &[fireball, charge, focus, shot, bolt],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: world.resource::<Behaviours>(),
        },
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
            self.server.write(AbilityRequest {
                actor: self.there,
                ..request
            });
        }
        let aimed: Vec<AbilityRequestWith<Aim>> = self
            .client
            .world_mut()
            .resource_mut::<Messages<AbilityRequestWith<Aim>>>()
            .drain()
            .collect();
        for request in aimed {
            self.server.write(AbilityRequestWith {
                actor: self.there,
                ..request
            });
        }
        let cancels: Vec<AbilityCancelRequest> = self
            .client
            .world_mut()
            .resource_mut::<Messages<AbilityCancelRequest>>()
            .drain()
            .collect();
        for cancel in cancels {
            self.server.write(AbilityCancelRequest {
                actor: self.there,
                ..cancel
            });
        }
        let inputs: Vec<AbilityInput<Aim>> = self
            .client
            .world_mut()
            .resource_mut::<Messages<AbilityInput<Aim>>>()
            .drain()
            .collect();
        for input in inputs {
            self.server.write(AbilityInput {
                actor: self.there,
                ..input
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
            self.client.write(AbilityReply {
                actor: self.here,
                ..reply
            });
        }
        let ends: Vec<AbilityEnd> = self
            .server
            .world_mut()
            .resource_mut::<Messages<AbilityEnd>>()
            .drain()
            .collect();
        for end in ends {
            self.client.write(AbilityEnd {
                actor: self.here,
                ..end
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
            .filter_map(|effect| {
                self.server
                    .entity(effect)
                    .try_get::<EffectReplica>()
                    .cloned()
            })
            .collect();
        for replica in effects {
            self.client.spawn((replica, EffectOf(self.here)));
        }
        self.client.tick();
    }

    /// The mage's granted `id`, on the client.
    fn ability_here(&self, id: &str) -> Entity {
        self.client
            .entity(self.here)
            .get::<Abilities>()
            .iter()
            .find(|ability| {
                self.client
                    .entity(*ability)
                    .try_get::<GrantedAbility>()
                    .is_some_and(|granted| granted.id().as_str() == id)
            })
            .unwrap()
    }

    /// The mage's granted `id`, on the server.
    fn ability_there(&self, id: &str) -> Entity {
        self.server
            .entity(self.there)
            .get::<Abilities>()
            .iter()
            .find(|ability| {
                self.server
                    .entity(*ability)
                    .try_get::<GrantedAbility>()
                    .is_some_and(|granted| granted.id().as_str() == id)
            })
            .unwrap()
    }

    fn cooldowns_here(&self) -> Vec<(bool, bool)> {
        self.client
            .entity(self.here)
            .try_get::<ActiveEffects>()
            .map(|effects| effects.iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|effect| {
                (
                    self.client.entity(effect).has::<PredictedUnder>(),
                    self.client.entity(effect).has::<Proxy>(),
                )
            })
            .collect()
    }
}

fn owns(app: &TestApp, actor: Entity, tag: &'static str) -> bool {
    app.entity(actor)
        .get::<OwnedTags>()
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
fn what_a_cost_is_routed_into_is_predicted_and_undone_if_rejected() {
    let mut game = Game::with(&["bolt"], &[]);
    game.client
        .world_mut()
        .activate_ability(game.here, "bolt")
        .unwrap();
    game.client.tick();
    assert!(close(get::<Mana>(&game.client, game.here), 10.0));
    assert!(
        close(get::<Strength>(&game.client, game.here), 11.0),
        "paid back"
    );

    // The server stuns the mage first: it refuses, and both are undone.
    own(&mut game.server, game.there, "state.stunned", true);
    game.server.tick();
    game.ask();
    game.answer();
    assert!(close(get::<Mana>(&game.client, game.here), 30.0));
    assert!(close(get::<Strength>(&game.client, game.here), 10.0));
}

#[test]
fn what_a_cost_is_routed_into_is_confirmed_as_the_server_has_it() {
    let mut game = Game::with(&["bolt"], &[]);
    game.client
        .world_mut()
        .activate_ability(game.here, "bolt")
        .unwrap();
    game.ask();
    assert!(close(get::<Mana>(&game.server, game.there), 10.0));
    assert!(close(get::<Strength>(&game.server, game.there), 11.0));
    game.answer();
    assert!(close(get::<Mana>(&game.client, game.here), 10.0));
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
    assert!(!game.client.entity(ability).has::<ActiveAbility>(), "ended");
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
    game.client.entity_mut(shown).despawn();
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
        .entity(game.here)
        .get::<ActiveEffects>()
        .iter()
        .find(|shown| game.client.entity(*shown).has::<Proxy>())
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
            .entity(ward)
            .try_get::<ActiveEffect>()
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
    game.server.write(AbilityRequest {
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
        game.client.entity(ability).get::<GrantedAbility>().level(),
        3.0
    );
}

#[test]
fn a_client_predicts_with_its_input_and_the_server_runs_with_the_same() {
    let mut game = Game::with(&["shot"], &[]);
    let here = game
        .client
        .world_mut()
        .activate_ability_with::<Shoot>(game.here, "shot", Aim(4.0))
        .unwrap();
    game.client.entity(here).assert_eq(Aim(4.0));
    game.ask();
    let there = game.ability_there("shot");
    game.server
        .entity(there)
        .assert_has::<Shoot>()
        .assert_eq(Aim(4.0));
    game.answer();
    game.client.entity(here).assert_has::<Shoot>();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "paid once"
    );
}

#[test]
fn input_the_server_rejects_undoes_the_prediction() {
    let mut game = Game::with(&["shot"], &[]);
    game.client.watch_triggered::<AbilityFailed>();
    let here = game
        .client
        .world_mut()
        .activate_ability_with::<Shoot>(game.here, "shot", Aim(40.0))
        .unwrap();
    game.client.tick();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "predicted"
    );
    game.ask();
    game.server
        .entity(game.ability_there("shot"))
        .assert_lacks::<Shoot>();
    assert!(
        close(get::<Mana>(&game.server, game.there), 30.0),
        "never paid"
    );
    game.answer();
    game.client
        .entity(here)
        .assert_lacks::<Shoot>()
        .assert_lacks::<Aim>();
    assert!(
        close(get::<Mana>(&game.client, game.here), 30.0),
        "mana back"
    );
    game.client
        .assert_triggered::<AbilityFailed>(|failed| failed.reason == Failure::Input);
}

#[test]
fn new_input_is_predicted_and_cancels_both_sides_if_the_server_rejects_it() {
    let mut game = Game::with(&["shot"], &[]);
    let here = game
        .client
        .world_mut()
        .activate_ability_with::<Shoot>(game.here, "shot", Aim(4.0))
        .unwrap();
    game.ask();
    game.answer();
    let there = game.ability_there("shot");

    // Aimed again, within reach: taken.
    game.client
        .world_mut()
        .set_input::<Shoot>(here, Aim(-8.0))
        .unwrap();
    game.client.entity(here).assert_eq(Aim(-8.0));
    game.ask();
    game.server.entity(there).assert_eq(Aim(-8.0));
    game.answer();

    // Aimed past what the server knows the shot reaches: predicted, then
    // cancelled on both sides.
    game.client
        .world_mut()
        .set_input::<Shoot>(here, Aim(50.0))
        .unwrap();
    game.client.entity(here).assert_eq(Aim(50.0));
    game.ask();
    game.server.entity(there).assert_lacks::<Shoot>();
    game.answer();
    game.client
        .entity(here)
        .assert_lacks::<Shoot>()
        .assert_lacks::<Aim>();
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "paid all the same: it ran"
    );
}

#[test]
fn a_cast_the_server_interrupts_ends_on_the_client_too() {
    let mut game = Game::new();
    game.client.watch_triggered::<AbilityEnded>();
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.ask();
    game.answer();

    // The server stuns the mage; the client doesn't see the stun.
    own(&mut game.server, game.there, "state.stunned", true);
    game.server.tick();
    game.server
        .entity(game.ability_there("fireball"))
        .assert_lacks::<ActiveAbility>();
    game.answer();
    game.client.entity(here).assert_lacks::<ActiveAbility>();
    game.client
        .assert_triggered::<AbilityEnded>(|ended| ended.ability == here && ended.cancelled);
    assert!(
        close(get::<Mana>(&game.client, game.here), 10.0),
        "paid all the same"
    );
}

/// A fireball cast by the client and served, and its ability on each side.
fn casting(game: &mut Game) -> (Entity, Entity) {
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.ask();
    game.answer();
    let there = game.ability_there("fireball");
    game.server.entity(there).assert_has::<ActiveAbility>();
    (here, there)
}

#[test]
fn a_cast_the_client_cancels_is_cancelled_on_the_server_too() {
    let mut game = Game::new();
    game.server.watch_triggered::<AbilityEnded>();
    let (here, there) = casting(&mut game);
    game.client.world_mut().cancel_ability(here);
    game.ask();
    game.server.entity(there).assert_lacks::<ActiveAbility>();
    game.server
        .assert_triggered::<AbilityEnded>(|ended| ended.ability == there && ended.cancelled);
}

#[test]
fn a_cast_the_client_ends_runs_its_course_on_the_server() {
    let mut game = Game::new();
    let (here, there) = casting(&mut game);
    game.client.world_mut().end_ability(here);
    game.ask();
    game.server.entity(there).assert_has::<ActiveAbility>();
}

#[test]
fn a_client_leaves_interrupting_to_the_server() {
    let mut game = Game::new();
    let (here, there) = casting(&mut game);

    // The client sees a stun the server doesn't: what it predicts may be
    // wrong, so its cast runs on.
    own(&mut game.client, game.here, "state.stunned", true);
    game.client.tick();
    game.client.entity(here).assert_has::<ActiveAbility>();
    game.ask();
    game.server.entity(there).assert_has::<ActiveAbility>();
}

#[test]
fn a_client_that_cant_pay_from_what_it_predicts_leaves_it_to_the_server() {
    let mut game = Game::with(&["charge"], &[]);
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "charge")
        .unwrap();
    game.ask();
    game.answer();
    let there = game.ability_there("charge");

    // The client predicts too little mana; the server has enough.
    game.client
        .world_mut()
        .get_mut::<Base<Mana>>(game.here)
        .unwrap()
        .set(5.0);
    game.client.tick();
    assert!(!game.client.world_mut().commit_ability(here));
    game.client.entity(here).assert_has::<ActiveAbility>();

    // The server can't pay either: it cancels, and tells the client.
    game.server
        .world_mut()
        .get_mut::<Base<Mana>>(game.there)
        .unwrap()
        .set(5.0);
    game.server.tick();
    assert!(!game.server.world_mut().commit_ability(there));
    game.server.entity(there).assert_lacks::<ActiveAbility>();
    game.answer();
    game.client.entity(here).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_cancel_that_overtakes_its_cast_is_held_for_it() {
    let mut game = Game::new();
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.world_mut().cancel_ability(here);

    // The cancel arrives a frame before the request.
    let request: Vec<AbilityRequest> = game
        .client
        .world_mut()
        .resource_mut::<Messages<AbilityRequest>>()
        .drain()
        .collect();
    game.ask();
    game.client.world_mut().write_message_batch(request);
    game.ask();
    game.server
        .entity(game.ability_there("fireball"))
        .assert_lacks::<ActiveAbility>();
}

#[test]
fn a_cancel_held_past_its_time_is_let_go() {
    let mut game = Game::new();
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.world_mut().cancel_ability(here);
    let request: Vec<AbilityRequest> = game
        .client
        .world_mut()
        .resource_mut::<Messages<AbilityRequest>>()
        .drain()
        .collect();
    game.ask();

    // The request comes long after: the cancel's past, and the cast runs.
    game.server.secs(1.5);
    game.client.world_mut().write_message_batch(request);
    game.ask();
    game.server
        .entity(game.ability_there("fireball"))
        .assert_has::<ActiveAbility>();
}

#[test]
fn a_cancel_asked_with_the_cast_follows_it_to_the_server() {
    let mut game = Game::new();
    let here = game
        .client
        .world_mut()
        .activate_ability(game.here, "fireball")
        .unwrap();
    game.client.world_mut().cancel_ability(here);
    game.ask();
    game.server
        .entity(game.ability_there("fireball"))
        .assert_lacks::<ActiveAbility>();
}
