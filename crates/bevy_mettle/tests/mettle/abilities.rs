//! Abilities: granted, activated, paid for, cooled down, blocked,
//! cancelled, triggered, committed by hand, and granted by effects.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Mana, actor, attributes, close, get, own};

/// The behaviour every ability here is run by.
#[derive(Component, Default)]
struct Cast;

const TAGS: &[&str] = &[
    "damage",
    "ability",
    "ability.spell",
    "ability.spell.fire",
    "ability.channel",
    "state",
    "state.casting",
    "state.stunned",
    "state.silenced",
    "state.raging",
    "cooldown",
    "cooldown.fireball",
    "event",
    "event.parried",
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

fn effects() -> Vec<Effect> {
    vec![
        Effect::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
        Effect::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
        Effect::infinite("staff"),
    ]
}

fn fireball() -> Ability {
    Ability::new("fireball", "cast")
        .tag("ability.spell.fire")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .grant("state.casting")
        .blocked_by("state.stunned")
}

fn channel() -> Ability {
    Ability::new("channel", "cast")
        .tag("ability.channel")
        .blocks("ability.spell")
}

/// An app with abilities `abilities` (and `channel`, which the staff
/// grants), and the effects they name.
fn app(abilities: &[Ability]) -> TestApp {
    let mut abilities = abilities.to_vec();
    if !abilities
        .iter()
        .any(|ability| ability.id.as_str() == "channel")
    {
        abilities.push(channel());
    }
    let mut app = TestApp::builder()
        .with(attributes)
        .plugins((EffectPlugin::default(), AbilityPlugin))
        .with(|app| {
            app.add_behaviour::<Cast>("cast");
        })
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::build(
        &effects(),
        &tags(),
        &CurveTable::default(),
        &attributes,
        &EffectCode::new(),
    )
    .unwrap();
    let abilities = AbilityRegistry::build(
        &abilities,
        &[Grant::new("staff", ["channel"])],
        &tags(),
        &effects,
        world.resource::<Behaviours>(),
    )
    .unwrap();
    world.insert_resource(effects);
    world.insert_resource(abilities);
    app
}

/// `actor`, granted every one of `abilities`.
fn caster(app: &mut TestApp, abilities: &[&'static str]) -> Entity {
    let actor = actor(app, []);
    for ability in abilities {
        app.world_mut().grant_ability(actor, *ability, 1.0);
    }
    actor
}

fn activate(app: &mut TestApp, actor: Entity, ability: &'static str) -> Result<Entity, Failure> {
    let activated = app.world_mut().activate_ability(actor, ability);
    app.tick();
    activated
}

fn owns(app: &TestApp, actor: Entity, tag: &'static str) -> bool {
    app.world()
        .get::<OwnedTags>(actor)
        .unwrap()
        .contains(&Id::from_static(tag))
}

#[test]
fn activating_runs_the_behaviour_pays_and_cools_down() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    assert!(
        app.world().get::<Cast>(ability).is_some(),
        "its behaviour runs"
    );
    assert_eq!(
        app.world().get::<ActiveAbility>(ability).unwrap().actor,
        mage
    );
    assert!(close(get::<Mana>(&app, mage), 10.0), "paid");
    assert!(owns(&app, mage, "state.casting"));
    app.world_mut().end_ability(ability, false);
    app.tick();
    assert!(app.world().get::<Cast>(ability).is_none());
    assert!(!owns(&app, mage, "state.casting"));
    assert_eq!(activate(&mut app, mage, "fireball"), Err(Failure::Cooldown));
    app.secs(1.0);
    assert_eq!(
        activate(&mut app, mage, "fireball"),
        Err(Failure::Cost),
        "10 mana left"
    );
}

#[test]
fn an_ability_is_blocked_by_its_owners_tags_and_active_abilities() {
    let mut app = app(&[fireball(), channel()]);
    let mage = caster(&mut app, &["fireball", "channel"]);
    own(&mut app, mage, "state.stunned", true);
    assert_eq!(activate(&mut app, mage, "fireball"), Err(Failure::Blocked));
    own(&mut app, mage, "state.stunned", false);
    activate(&mut app, mage, "channel").unwrap();
    assert_eq!(
        activate(&mut app, mage, "fireball"),
        Err(Failure::Blocked),
        "channelling"
    );
    assert_eq!(activate(&mut app, mage, "channel"), Err(Failure::Active));
}

#[test]
fn activating_cancels_what_it_cancels() {
    let interrupt = Ability::new("interrupt", "cast").cancels("ability.channel");
    let mut app = app(&[channel(), interrupt]);
    let mage = caster(&mut app, &["channel", "interrupt"]);
    let channelling = activate(&mut app, mage, "channel").unwrap();
    activate(&mut app, mage, "interrupt").unwrap();
    assert!(app.world().get::<ActiveAbility>(channelling).is_none());
}

#[test]
fn a_requirement_has_to_hold() {
    let spell = Ability::new("spell", "cast").require(!Condition::owner_has("state.silenced"));
    let mut app = app(&[spell]);
    let mage = caster(&mut app, &["spell"]);
    own(&mut app, mage, "state.silenced", true);
    assert_eq!(activate(&mut app, mage, "spell"), Err(Failure::Requirement));
    assert_eq!(
        activate(&mut app, mage, "missing"),
        Err(Failure::NotGranted)
    );
}

#[test]
fn a_manual_ability_pays_when_its_behaviour_commits() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    assert!(close(get::<Mana>(&app, mage), 30.0), "not paid yet");
    assert!(app.world_mut().commit_ability(ability));
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 10.0));
    assert!(
        app.world_mut().commit_ability(ability),
        "once per activation"
    );
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 10.0));
}

#[test]
fn a_commit_that_cant_pay_cancels() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    app.world_mut()
        .get_mut::<Base<Mana>>(mage)
        .unwrap()
        .set(5.0);
    app.tick();
    assert!(!app.world_mut().commit_ability(ability));
    assert!(app.world().get::<ActiveAbility>(ability).is_none());
}

#[test]
fn triggers_activate_on_events_and_owned_tags() {
    let riposte = Ability::new("riposte", "cast").trigger(Trigger::Event(Id::from_static("event")));
    let rage =
        Ability::new("rage", "cast").trigger(Trigger::TagPresent(Id::from_static("state.raging")));
    let mut app = app(&[riposte, rage]);
    let knight = caster(&mut app, &["riposte", "rage"]);
    let foe = actor(&mut app, []);
    let mut parried = GameplayEvent::new(knight, "event.parried");
    parried.instigator = Some(foe);
    app.world_mut().send_event(parried);
    app.tick();
    let abilities: Vec<Entity> = app
        .world()
        .get::<Abilities>(knight)
        .unwrap()
        .iter()
        .collect();
    let active = app.world().get::<ActiveAbility>(abilities[0]).unwrap();
    assert_eq!(
        active.event.as_ref().unwrap().instigator,
        Some(foe),
        "with the event"
    );
    own(&mut app, knight, "state.raging", true);
    app.ticks(2);
    assert!(
        app.world().get::<ActiveAbility>(abilities[1]).is_some(),
        "the tag present"
    );
    own(&mut app, knight, "state.raging", false);
    app.ticks(2);
    assert!(
        app.world().get::<ActiveAbility>(abilities[1]).is_none(),
        "and gone"
    );
}

#[test]
fn abilities_activate_by_tag() {
    let mut app = app(&[fireball(), channel()]);
    let mage = caster(&mut app, &["fireball", "channel"]);
    app.world_mut()
        .commands()
        .entity(mage)
        .activate_tagged([Id::from_static("ability.spell")].into_iter().collect());
    app.tick();
    let active: Vec<bool> = app
        .world()
        .get::<Abilities>(mage)
        .unwrap()
        .iter()
        .map(|ability| app.world().get::<ActiveAbility>(ability).is_some())
        .collect();
    assert_eq!(active, [true, false]);
}

#[test]
fn an_effect_grants_abilities_while_it_lasts() {
    let mut app = app(&[channel()]);
    let mage = actor(&mut app, []);
    app.world_mut().apply_effect(mage, EffectSpec::new("staff"));
    app.tick();
    let staff = app
        .world()
        .get::<ActiveEffects>(mage)
        .unwrap()
        .iter()
        .next()
        .unwrap();
    activate(&mut app, mage, "channel").unwrap();
    app.world_mut().remove_effect(staff);
    app.tick();
    assert!(
        app.world()
            .get::<Abilities>(mage)
            .is_none_or(|abilities| abilities.iter().count() == 0)
    );
}

#[test]
fn a_cooldown_applied_under_an_activation_can_be_taken_back() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    let activation = app
        .world()
        .get::<ActiveAbility>(ability)
        .unwrap()
        .activation;
    app.world_mut().end_ability(ability, true);
    app.world_mut()
        .commands()
        .entity(mage)
        .remove_activation(activation);
    app.tick();
    assert!(
        !owns(&app, mage, "cooldown.fireball"),
        "a rejected prediction"
    );
}

#[test]
fn an_ability_reads_from_data_as_written() {
    let def: Ability = ron::from_str(
        r#"(
            id: "fireball",
            tags: ["ability.spell.fire"],
            behaviour: "cast",
            cost: "fireball.cost",
            cooldown: "fireball.cooldown",
            require: Not(OwnerHas("state.silenced")),
            blocked_by: ["state.stunned"],
            triggers: [Event("event.parried")],
            commit: Manual,
        )"#,
    )
    .unwrap();
    let app = app(&[def]);
    assert!(
        app.world()
            .resource::<AbilityRegistry>()
            .get(&Id::from_static("fireball"))
            .is_some()
    );
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    let mut app = TestApp::builder()
        .with(attributes)
        .plugins((EffectPlugin::default(), AbilityPlugin))
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::build(
        &effects(),
        &tags(),
        &CurveTable::default(),
        &attributes,
        &EffectCode::new(),
    )
    .unwrap();
    let bad = Ability::new("bad", "nowhere")
        .tag("ability.unknown")
        .cost("fireball.cooldown")
        .cooldown("missing");
    let grants = [
        Grant::new("staff", ["channel"]),
        Grant::new("staf", ["channel"]),
        Grant::new("fireball.cost", ["bad"]),
    ];
    let errors = AbilityRegistry::build(
        &[bad],
        &grants,
        &tags(),
        &effects,
        world.resource::<Behaviours>(),
    )
    .unwrap_err()
    .to_string();
    for expected in [
        "there's no behaviour called `nowhere` (in `bad`)",
        "there's no tag called `ability.unknown` (in `bad`)",
        "a cost of `fireball.cooldown`: it isn't instant (in `bad`)",
        "there's no effect called `missing` (in `bad`)",
        "there's no ability called `channel` (in `staff`)",
        "there's no effect called `staf` (in `staf`)",
        "a grant of `fireball.cost`: it's instant, so it grants nothing for any while",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}
