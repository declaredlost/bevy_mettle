//! The game's content, handed over as one resource: checked as a whole,
//! loaded or rejected, and reloaded as it changes.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::ability::behaviour::AnyBehaviour;
use bevy_mettle::core::registry::AnyAttribute;
use bevy_mettle::effect::code::{AnyCalculation, AnyExecution, AnyRoute};
use bevy_mettle::prelude::*;

use crate::common::{Life, Mana, attributes, close, mettle};

#[derive(Component, Default, Behaviour)]
struct Cast;

/// The fireball, as code names it.
#[derive(Ability)]
struct Fireball;

/// The burn, as code names it: an effect, and the execution it runs.
#[derive(Effect, Execution)]
struct Burn;

impl ExecutionFn for Burn {
    fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
        vec![BaseChange::of::<Life>(Operation::Flat, -5.0)]
    }
}

/// The fireball, its cost and cooldown, and a burn run in code.
fn content() -> MettleContent {
    MettleContent {
        tags: ["cooldown", "cooldown.fireball"]
            .into_iter()
            .map(TagKey::named)
            .collect(),
        effects: vec![
            EffectData::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            EffectData::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
            EffectData::instant(Key::of::<Burn>()).execution(Key::of::<Burn>()),
        ],
        abilities: vec![
            AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>())
                .cost("fireball.cost")
                .cooldown("fireball.cooldown"),
        ],
        ..default()
    }
}

/// The behaviour and the code `content` names.
fn code(app: &mut App) -> &mut App {
    app.add_behaviour::<Cast>()
        .add_ability::<Fireball>()
        .add_effect::<Burn>()
        .add_execution(Burn)
}

/// An app with the attributes, the behaviour and the code `content` names.
fn app(content: MettleContent) -> TestApp {
    TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(|app| {
            code(app).insert_resource(content);
        })
        .watch_triggered::<ContentLoaded>()
        .watch_triggered::<ContentRejected>()
        .build()
}

#[test]
fn content_is_loaded_before_anything_runs() {
    let mut app = app(content());
    app.assert_triggered::<ContentLoaded>(|_| true);
    app.assert_triggered_none::<ContentRejected>(|_| true);
    assert!(app.resource::<TagRegistry>().contains("cooldown.fireball"));
    let effects = app.resource::<EffectRegistry>();
    assert!(
        effects
            .id_of::<Burn>()
            .is_some_and(|burn| effects.get(burn).is_some())
    );
    let abilities = app.resource::<AbilityRegistry>();
    assert!(
        abilities
            .id_of::<Fireball>()
            .is_some_and(|fireball| abilities.get(fireball).is_some())
    );

    let mage = app.spawn((Base::<Mana>::default(), Base::<Life>::default()));
    app.world_mut().grant_ability(mage, Fireball).unwrap();
    app.world_mut().activate_ability(mage, Fireball).unwrap();
    app.world_mut().apply_effect(mage, Burn);
    app.tick();
    app.entity(mage)
        .assert::<Current<Mana>>(|mana| close(mana.get(), 10.0))
        .assert::<Current<Life>>(|life| close(life.get(), 45.0));
}

#[test]
fn wrong_content_is_rejected_and_the_content_before_stays() {
    let mut app = app(content());
    let mut wrong = content();
    wrong
        .effects
        .push(EffectData::lasting("frozen", 1.0).grant("state.frozen"));
    wrong.abilities.push(AbilityData::new("blink", "teleport"));
    app.insert_resource(wrong);
    app.tick();
    app.assert_triggered::<ContentRejected>(|rejected| {
        rejected.errors.to_string().contains("state.frozen")
    });
    assert!(
        app.resource::<EffectRegistry>().id("frozen").is_none(),
        "the content before stays"
    );
    let abilities = app.resource::<AbilityRegistry>();
    assert!(
        abilities
            .id_of::<Fireball>()
            .is_some_and(|fireball| abilities.get(fireball).is_some())
    );
}

#[test]
fn changed_content_is_reloaded_as_a_whole() {
    let mut app = app(content());
    app.read_triggered::<ContentLoaded>();
    let mage = app.spawn(Base::<Mana>::default());
    app.world_mut().grant_ability(mage, Fireball).unwrap();

    // A cheaper fireball.
    app.resource_mut::<MettleContent>().effects[0] = EffectData::instant("fireball.cost")
        .modifier(EffectModifier::new::<Mana>(Operation::Flat, -5.0));
    app.tick();
    app.assert_triggered::<ContentLoaded>(|_| true);
    app.world_mut().activate_ability(mage, Fireball).unwrap();
    app.tick();
    app.entity(mage)
        .assert::<Current<Mana>>(|mana| close(mana.get(), 25.0));
}

#[test]
fn content_can_be_checked_at_once_without_loading_it() {
    let app = app(MettleContent::default());
    let mut wrong = content();
    wrong.abilities.push(AbilityData::new("blink", "teleport"));
    let errors = wrong.check(app.world()).unwrap_err().to_string();
    assert!(errors.contains("teleport"), "{errors}");
    content().check(app.world()).unwrap();
}

#[test]
fn content_left_out_is_empty() {
    use serde_test::{Token, assert_de_tokens};

    let content = MettleContent {
        tags: vec![TagKey::named("cooldown")],
        ..default()
    };
    assert_de_tokens(
        &content,
        &[
            Token::Struct {
                name: "MettleContent",
                len: 1,
            },
            Token::Str("tags"),
            Token::Seq { len: Some(1) },
            Token::Str("cooldown"),
            Token::SeqEnd,
            Token::StructEnd,
        ],
    );
}

#[test]
fn content_inserted_as_the_app_is_built_is_there_for_startup() {
    fn arm(mut commands: Commands) {
        commands
            .spawn(Base::<Mana>::default())
            .grant_ability(Fireball)
            .activate_ability(Fireball);
    }
    let mut app = TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(|app| {
            code(app)
                .insert_resource(content())
                .add_systems(Startup, arm);
        })
        .watch_triggered::<ContentLoaded>()
        .build();
    assert_eq!(
        app.read_triggered::<ContentLoaded>().len(),
        1,
        "loaded once"
    );
    app.tick();
    app.query::<&Current<Mana>, ()>()
        .assert_all(|mana| close(mana.get(), 10.0));
    app.query::<(), With<Cast>>().assert_len(1);
}

/// The rite, as code names it: an ability, and the effect it costs, named
/// apart.
#[derive(Ability, Effect)]
#[effect(name = "rite.cost")]
struct Rite;

/// A calculation, as code names it.
#[derive(Calculation)]
struct BurnTime;

/// A route, named otherwise.
#[derive(Route)]
#[route(name = "incoming.fire")]
struct IncomingFire;

#[test]
fn one_type_is_keyed_as_each_kind_it_derives() {
    let ability: Key<AnyAbility> = Key::of::<Rite>();
    let effect: Key<AnyEffect> = Key::of::<Rite>();
    assert_eq!(ability.as_str(), "rite");
    assert_eq!(effect.as_str(), "rite.cost");

    let content = MettleContent {
        effects: vec![
            EffectData::instant(Key::of::<Rite>())
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -5.0)),
        ],
        abilities: vec![
            AbilityData::new(Key::of::<Rite>(), Key::of::<Cast>()).cost(Key::of::<Rite>()),
        ],
        ..default()
    };
    let app = TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(|app| {
            app.add_behaviour::<Cast>()
                .add_ability::<Rite>()
                .add_effect::<Rite>()
                .insert_resource(content.clone());
        })
        .watch_triggered::<ContentLoaded>()
        .build();
    content.check(app.world()).unwrap();
    app.assert_triggered::<ContentLoaded>(|_| true);

    let cost = app.resource::<EffectRegistry>().id_of::<Rite>();
    assert!(cost.is_some(), "the effect is loaded by its own name");
    let abilities = app.resource::<AbilityRegistry>();
    let rite = abilities
        .id_of::<Rite>()
        .and_then(|rite| abilities.get(rite));
    assert_eq!(
        rite.map(|rite| rite.cost),
        Some(cost),
        "it costs the effect"
    );
}

#[test]
fn each_derive_is_keyed_by_its_name() {
    assert_eq!(
        Key::<AnyEffect>::of::<Burn>().as_str(),
        <Burn as Effect>::NAME
    );
    assert_eq!(
        Key::<AnyAbility>::of::<Fireball>().as_str(),
        <Fireball as Ability>::NAME
    );
    assert_eq!(Key::<AnyBehaviour>::of::<Cast>().as_str(), Cast::NAME);
    assert_eq!(Key::<AnyAttribute>::of::<Life>().as_str(), Life::NAME);
    assert_eq!(
        Key::<AnyExecution>::of::<Burn>().as_str(),
        <Burn as Execution>::NAME
    );
    assert_eq!(
        Key::<AnyCalculation>::of::<BurnTime>().as_str(),
        BurnTime::NAME
    );
    assert_eq!(
        Key::<AnyRoute>::of::<IncomingFire>().as_str(),
        IncomingFire::NAME
    );

    // An explicit name is the key.
    assert_eq!(Key::<AnyEffect>::of::<Rite>().as_str(), "rite.cost");
    assert_eq!(
        Key::<AnyRoute>::of::<IncomingFire>().as_str(),
        "incoming.fire"
    );
    assert_eq!(
        Key::<AnyCalculation>::of::<BurnTime>().as_str(),
        "burn_time"
    );
}
