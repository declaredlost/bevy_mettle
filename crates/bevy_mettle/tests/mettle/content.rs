//! The game's content, handed over as one resource: checked as a whole,
//! loaded or rejected, and reloaded as it changes.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Life, Mana, attributes, close, mettle};

#[derive(Component, Default, Behaviour)]
struct Cast;

/// The fireball, its cost and cooldown, and a burn run in code.
fn content() -> MettleContent {
    MettleContent {
        tags: ["cooldown", "cooldown.fireball"]
            .into_iter()
            .map(|id| Tag { id: Id::new(id) })
            .collect(),
        effects: vec![
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            Effect::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
            Effect::instant("burn").execution("burn"),
        ],
        abilities: vec![
            Ability::new("fireball", "cast")
                .cost("fireball.cost")
                .cooldown("fireball.cooldown"),
        ],
        ..default()
    }
}

/// The behaviour and the code `content` names.
fn code(app: &mut App) -> &mut App {
    app.add_behaviour::<Cast>()
        .add_execution("burn", |_: &EffectContext| {
            vec![BaseChange::new::<Life>(Operation::Flat, -5.0)]
        })
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
    assert!(
        app.resource::<TagTree>()
            .contains(&Id::new("cooldown.fireball"))
    );
    assert!(
        app.resource::<EffectRegistry>()
            .get(&Id::new("burn"))
            .is_some()
    );
    assert!(
        app.resource::<AbilityRegistry>()
            .get(&Id::new("fireball"))
            .is_some()
    );

    let mage = app.spawn((Base::<Mana>::default(), Base::<Life>::default()));
    app.world_mut()
        .grant_ability(mage, "fireball", 1.0)
        .unwrap();
    app.world_mut().activate_ability(mage, "fireball").unwrap();
    app.world_mut().apply_effect(mage, EffectSpec::new("burn"));
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
        .push(Effect::lasting("frozen", 1.0).grant("state.frozen"));
    wrong.abilities.push(Ability::new("blink", "teleport"));
    app.insert_resource(wrong);
    app.tick();
    app.assert_triggered::<ContentRejected>(|rejected| {
        rejected.errors.to_string().contains("state.frozen")
    });
    assert!(
        app.resource::<EffectRegistry>()
            .get(&Id::new("frozen"))
            .is_none(),
        "the content before stays"
    );
    assert!(
        app.resource::<AbilityRegistry>()
            .get(&Id::new("fireball"))
            .is_some()
    );
}

#[test]
fn changed_content_is_reloaded_as_a_whole() {
    let mut app = app(content());
    app.read_triggered::<ContentLoaded>();
    let mage = app.spawn(Base::<Mana>::default());
    app.world_mut()
        .grant_ability(mage, "fireball", 1.0)
        .unwrap();

    // A cheaper fireball.
    app.resource_mut::<MettleContent>().effects[0] = Effect::instant("fireball.cost")
        .modifier(EffectModifier::new::<Mana>(Operation::Flat, -5.0));
    app.tick();
    app.assert_triggered::<ContentLoaded>(|_| true);
    app.world_mut().activate_ability(mage, "fireball").unwrap();
    app.tick();
    app.entity(mage)
        .assert::<Current<Mana>>(|mana| close(mana.get(), 25.0));
}

#[test]
fn content_can_be_checked_at_once_without_loading_it() {
    let app = app(MettleContent::default());
    let mut wrong = content();
    wrong.abilities.push(Ability::new("blink", "teleport"));
    let errors = wrong.check(app.world()).unwrap_err().to_string();
    assert!(errors.contains("teleport"), "{errors}");
    content().check(app.world()).unwrap();
}

#[test]
fn content_reads_as_written() {
    let content: MettleContent = ron::from_str(
        r#"(
            tags: [(id: "cooldown"), (id: "cooldown.fireball")],
            effects: [(id: "fireball.cooldown", duration: For(Value(2)), grants: ["cooldown.fireball"])],
        )"#,
    )
    .unwrap();
    assert_eq!(content.effects.len(), 1);
    assert!(content.abilities.is_empty(), "what isn't written is empty");
}

#[test]
fn content_inserted_as_the_app_is_built_is_there_for_startup() {
    fn arm(mut commands: Commands) {
        commands
            .spawn(Base::<Mana>::default())
            .grant_ability("fireball", 1.0)
            .activate_ability("fireball");
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
