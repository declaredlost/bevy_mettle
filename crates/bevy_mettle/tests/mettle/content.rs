//! The game's content, handed over as one resource: what it adds to what
//! code defines, checked as a whole, loaded or rejected, and reloaded as it
//! changes.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::Error;
use bevy_mettle::effect::EffectSpec;
use bevy_mettle::prelude::*;

use crate::common::{Life, Mana, attributes, close, mettle};

#[derive(Component, Default, Behaviour)]
struct Cast;

/// The fireball: 20 mana, a second's cooldown.
#[derive(Ability)]
#[ability(behaviour::<Cast>, cost = Cost::of::<Mana>(20.0), cooldown = 1.0)]
struct Fireball;

/// The burn, run in code.
#[derive(Effect)]
#[effect(execution::<BurnDamage>)]
struct Burn;

/// The execution the burn runs.
#[derive(Execution)]
struct BurnDamage;

impl ExecutionFn for BurnDamage {
    fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
        vec![BaseChange::of::<Life>(Operation::Flat, -5.0)]
    }
}

/// What a scorch takes, by its level: 4 at every level, unless tuned.
#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 4.0)])]
struct ScorchDamage;

/// The scorch, as hard as its curve says.
#[derive(Effect)]
#[effect(modifier(EffectModifier::new::<Life>(
    Operation::Flat,
    Magnitude::curve::<ScorchDamage>().times(-1.0),
)))]
struct Scorch;

/// What content adds: a frostbite and a blink, and the tags they name.
fn content() -> MettleContent {
    MettleContent {
        tags: ["modded", "modded.frost"]
            .into_iter()
            .map(TagKey::named)
            .collect(),
        effects: vec![frostbite(5.0)],
        abilities: vec![AbilityData::new("blink").behaviour::<Cast>()],
        ..default()
    }
}

/// The frostbite content adds, taking `life`.
fn frostbite(life: f32) -> EffectData {
    EffectData::new("frostbite")
        .tag("modded.frost")
        .flat::<Life>(-life)
}

/// What code defines: the behaviour, the fireball, the burn and the scorch.
fn code(app: &mut App) -> &mut App {
    app.add_behaviour::<Cast>()
        .add_ability::<Fireball>()
        .add_effects::<(Burn, Scorch)>()
        .add_execution(BurnDamage)
        .add_curve::<ScorchDamage>()
}

/// An app with the attributes and what code defines, and `content`, if any.
fn app(content: Option<MettleContent>) -> TestApp {
    TestApp::builder()
        .plugins(mettle())
        .with(attributes)
        .with(|app| {
            code(app);
            if let Some(content) = content {
                app.insert_resource(content);
            }
        })
        .watch_triggered::<ContentLoaded>()
        .watch_triggered::<ContentRejected>()
        .build()
}

/// An actor with mana and life.
fn mage(app: &mut TestApp) -> Entity {
    app.spawn((Base::<Mana>::default(), Base::<Life>::default()))
}

/// The effect content added as `name`, applied to `target`.
fn apply_named(app: &mut TestApp, target: Entity, name: &str) {
    let id = app
        .resource::<EffectRegistry>()
        .id(name)
        .expect("an effect content added");
    app.world_mut()
        .apply_effect(target, EffectSpec::from_id(id));
}

#[test]
fn what_code_defines_is_loaded_with_no_content() {
    let mut app = app(None);
    app.assert_triggered::<ContentLoaded>(|_| true);
    assert!(!app.world().contains_resource::<MettleContent>());
    let effects = app.resource::<EffectRegistry>();
    for name in ["burn", "scorch", "fireball.cost", "fireball.cooldown"] {
        assert!(effects.id(name).is_some(), "{name}");
    }
    assert!(app.resource::<TagRegistry>().contains("cooldown.fireball"));

    let mage = mage(&mut app);
    app.world_mut().grant_ability(mage, Fireball).unwrap();
    app.world_mut().activate_ability(mage, Fireball).unwrap();
    app.world_mut().apply_effect(mage, Burn);
    app.tick();
    app.entity(mage)
        .assert::<Current<Mana>>(|mana| close(mana.get(), 10.0))
        .assert::<Current<Life>>(|life| close(life.get(), 45.0));
}

#[test]
fn content_adds_effects_and_abilities_beside_codes() {
    let mut app = app(Some(content()));
    app.assert_triggered::<ContentLoaded>(|_| true);
    app.assert_triggered_none::<ContentRejected>(|_| true);
    assert!(app.resource::<TagRegistry>().contains("modded.frost"));
    let effects = app.resource::<EffectRegistry>();
    assert!(
        effects
            .id_of::<Burn>()
            .is_some_and(|burn| effects.get(burn).is_some()),
        "code's still there"
    );
    let abilities = app.resource::<AbilityRegistry>();
    assert!(
        abilities
            .id("blink")
            .is_some_and(|blink| abilities.get(blink).is_some())
    );
    assert!(abilities.id_of::<Fireball>().is_some());

    let mage = mage(&mut app);
    apply_named(&mut app, mage, "frostbite");
    app.world_mut().apply_effect(mage, Burn);
    app.tick();
    app.entity(mage)
        .assert::<Current<Life>>(|life| close(life.get(), 40.0));
}

#[test]
fn content_defining_what_code_defines_is_rejected() {
    let app = app(None);
    // Abilities are checked once the effects hold, so each on its own.
    let effect = MettleContent {
        effects: vec![EffectData::new(Key::of::<Burn>()).flat::<Life>(-50.0)],
        ..default()
    };
    let ability = MettleContent {
        abilities: vec![AbilityData::new(Key::of::<Fireball>()).behaviour::<Cast>()],
        ..default()
    };
    for (content, kind, name) in [(effect, "effect", "burn"), (ability, "ability", "fireball")] {
        let errors = content.check(app.world()).unwrap_err();
        let defined = Error::DefinedInCode {
            kind,
            name: name.to_owned(),
        };
        assert!(errors.0.contains(&defined), "{defined} not in: {errors}");
        assert!(
            errors.to_string().contains(&format!(
                "content defines {kind} `{name}`, which code defines"
            )),
            "{errors}"
        );
    }
}

#[test]
fn content_tunes_a_curve_code_defines() {
    let tuned = MettleContent {
        curves: vec![CurveData::new(Key::of::<ScorchDamage>()).keys([(1.0, 4.0), (5.0, 20.0)])],
        ..default()
    };
    // Tuned, and as code defines it.
    for (content, life) in [(Some(tuned), 30.0), (None, 46.0)] {
        let mut app = app(content);
        app.assert_triggered_none::<ContentRejected>(|_| true);
        let mage = mage(&mut app);
        app.world_mut().apply_effect(mage, Scorch.level(5.0));
        app.tick();
        app.entity(mage)
            .assert::<Current<Life>>(|current| close(current.get(), life));
    }
}

#[test]
fn wrong_content_is_rejected_and_the_content_before_stays() {
    let mut app = app(Some(content()));
    let mut wrong = content();
    wrong.effects.push(
        EffectData::new("frozen")
            .lasting(1.0)
            .owner_tag("state.frozen"),
    );
    wrong
        .abilities
        .push(AbilityData::new("warp").behaviour::<Cast>());
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
    assert!(abilities.id("warp").is_none());
    assert!(
        abilities
            .id("blink")
            .is_some_and(|blink| abilities.get(blink).is_some())
    );
}

#[test]
fn changed_content_is_reloaded_as_a_whole() {
    let mut app = app(Some(content()));
    app.read_triggered::<ContentLoaded>();
    let mage = mage(&mut app);

    // A harsher frostbite, and a scorch tuned harder.
    {
        let mut content = app.resource_mut::<MettleContent>();
        content.effects[0] = frostbite(15.0);
        content
            .curves
            .push(CurveData::new(Key::of::<ScorchDamage>()).key(1.0, 10.0));
    }
    app.tick();
    app.assert_triggered::<ContentLoaded>(|_| true);
    apply_named(&mut app, mage, "frostbite");
    app.world_mut().apply_effect(mage, Scorch);
    app.tick();
    app.entity(mage)
        .assert::<Current<Life>>(|life| close(life.get(), 25.0));
    assert!(
        app.resource::<EffectRegistry>().id_of::<Burn>().is_some(),
        "code's defined again"
    );
}

#[test]
fn content_can_be_checked_at_once_without_loading_it() {
    let app = app(None);
    let mut wrong = content();
    wrong.abilities.push(AbilityData {
        behaviour: Some(Key::from_static("teleport")),
        ..AbilityData::new("warp")
    });
    let errors = wrong.check(app.world()).unwrap_err().to_string();
    assert!(errors.contains("teleport"), "{errors}");
    content().check(app.world()).unwrap();
    assert!(
        app.resource::<EffectRegistry>().id("frostbite").is_none(),
        "only checked"
    );
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
    fn arm(mut commands: Commands, effects: Res<EffectRegistry>) {
        let frostbite = effects.id("frostbite").expect("content's");
        commands
            .spawn((Base::<Mana>::default(), Base::<Life>::default()))
            .grant_ability(Fireball)
            .activate_ability(Fireball)
            .apply_effect(EffectSpec::from_id(frostbite));
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
    app.query::<&Current<Life>, ()>()
        .assert_all(|life| close(life.get(), 45.0));
    app.query::<(), With<Cast>>().assert_len(1);
}

/// A calculation, as code names it.
#[derive(Calculation)]
struct BurnTime;

/// A route, named otherwise.
#[derive(Route)]
#[route(name = "incoming.fire")]
struct IncomingFire;

#[test]
fn each_derive_is_keyed_by_its_name() {
    // Its type's name in `snake_case`.
    assert_eq!(Key::of::<Burn>().as_str(), "burn");
    assert_eq!(BurnDamage::KEY, "burn_damage");
    assert_eq!(Key::of::<Fireball>().as_str(), "fireball");
    assert_eq!(Cast::KEY, "cast");
    assert_eq!(Life::KEY, "life");
    assert_eq!(BurnTime::KEY, "burn_time");
    assert_eq!(ScorchDamage::KEY, "scorch_damage");

    // An explicit name is the key.
    assert_eq!(Key::of::<IncomingFire>().as_str(), "incoming.fire");
}
