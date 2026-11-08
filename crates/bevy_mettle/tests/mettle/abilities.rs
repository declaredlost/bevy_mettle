//! Abilities: granted, activated, paid for, cooled down, blocked, cancelled,
//! triggered, committed by hand, and granted by effects.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{Mana, actor, attributes, close, get, own};

/// The behaviour every ability here is run by.
#[derive(Component, Default, Behaviour)]
struct Cast;

/// A behaviour that notes its caster's mana as it starts ([`witness`]).
#[derive(Component, Default, Behaviour)]
struct Witness;

/// The caster's mana as [`Witness`] started.
#[derive(Resource, Default)]
struct Witnessed(Option<f32>);

fn witness(add: On<Add<Witness>>, active: Query<&ActiveAbility>, mut commands: Commands) {
    let actor = active.get(add.entity).unwrap().actor;
    commands.queue(move |world: &mut World| {
        let mana = world.get::<Base<Mana>>(actor).map(Base::get);
        world.insert_resource(Witnessed(mana));
    });
}

/// Where a shot is aimed: how far along.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Aim(f32);

/// A shot being fired: aimed.
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct Shoot;

/// Whether `check`'s aim is within reach: 10 along, either way.
fn in_reach(In(check): In<InputCheck<Aim>>) -> bool {
    check.input.0.abs() <= 10.0
}

fn shot() -> Ability {
    Ability::new("shot", "shoot").cost("fireball.cost")
}

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
        Effect::instant("surge.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -30.0))
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -30.0)),
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

/// An app with abilities `abilities` (and `channel`, which the staff grants),
/// and the effects they name.
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
            app.add_behaviour::<Cast>()
                .add_behaviour::<Shoot>()
                .add_behaviour::<Witness>()
                .add_observer(witness)
                .check_input::<Aim, _>(in_reach);
        })
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::check(
        &effects(),
        EffectChecks {
            tags: &tags(),
            curves: &CurveTable::default(),
            attributes: &attributes,
            code: &EffectCode::new(),
        },
    )
    .unwrap();
    let abilities = AbilityRegistry::check(
        &abilities,
        &[Grant::new("staff", ["channel"])],
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
    app.entity(actor)
        .get::<OwnedTags>()
        .contains(&Id::from_static(tag))
}

#[test]
fn activating_runs_the_behaviour_pays_and_cools_down() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    assert!(app.entity(ability).has::<Cast>(), "its behaviour runs");
    assert_eq!(app.entity(ability).get::<ActiveAbility>().actor, mage);
    assert!(close(get::<Mana>(&app, mage), 10.0), "paid");
    assert!(owns(&app, mage, "state.casting"));
    app.world_mut().end_ability(ability);
    app.tick();
    app.entity(ability).assert_lacks::<Cast>();
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
    app.entity(channelling).assert_lacks::<ActiveAbility>();
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
    app.entity(ability).assert_lacks::<ActiveAbility>();
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
    let abilities: Vec<Entity> = app.entity(knight).get::<Abilities>().iter().collect();
    let active = app.entity(abilities[0]).get::<ActiveAbility>();
    assert_eq!(
        active.event.as_ref().unwrap().instigator,
        Some(foe),
        "with the event"
    );
    own(&mut app, knight, "state.raging", true);
    app.ticks(2);
    assert!(
        app.entity(abilities[1]).has::<ActiveAbility>(),
        "the tag present"
    );
    own(&mut app, knight, "state.raging", false);
    app.ticks(2);
    assert!(!app.entity(abilities[1]).has::<ActiveAbility>(), "and gone");
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
        .entity(mage)
        .get::<Abilities>()
        .iter()
        .map(|ability| app.entity(ability).has::<ActiveAbility>())
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
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    activate(&mut app, mage, "channel").unwrap();
    app.world_mut().remove_effect(staff);
    app.tick();
    assert!(
        app.entity(mage)
            .try_get::<Abilities>()
            .is_none_or(|abilities| abilities.iter().count() == 0)
    );
}

#[test]
fn a_cooldown_applied_under_an_activation_can_be_taken_back() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    let activation = app.entity(ability).get::<ActiveAbility>().activation;
    app.world_mut().cancel_ability(ability);
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
        app.resource::<AbilityRegistry>()
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
    let effects = EffectRegistry::check(
        &effects(),
        EffectChecks {
            tags: &tags(),
            curves: &CurveTable::default(),
            attributes: &attributes,
            code: &EffectCode::new(),
        },
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
    let errors = AbilityRegistry::check(
        &[bad],
        &grants,
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: world.resource::<Behaviours>(),
        },
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

#[test]
fn a_cost_is_checked_change_after_change() {
    let surge = Ability::new("surge", "cast").cost("surge.cost");
    let mut app = app(&[surge]);
    let mage = caster(&mut app, &["surge"]);
    app.world_mut()
        .get_mut::<Base<Mana>>(mage)
        .unwrap()
        .set(50.0);
    app.tick();
    assert_eq!(
        activate(&mut app, mage, "surge"),
        Err(Failure::Cost),
        "50 can't pay 30 twice"
    );
    assert!(close(get::<Mana>(&app, mage), 50.0));
}

#[test]
fn an_actor_spawned_this_frame_pays_from_its_bases() {
    let mut app = app(&[fireball()]);
    let rich = app.spawn(Base::<Mana>::default());
    let poor = app.spawn(Base::<Mana>::new(10.0));
    for mage in [rich, poor] {
        app.world_mut()
            .grant_ability(mage, "fireball", 1.0)
            .unwrap();
    }
    // Nothing worked out yet: no tick since they were spawned.
    assert!(app.world_mut().activate_ability(rich, "fireball").is_ok());
    assert_eq!(
        app.world_mut().activate_ability(poor, "fireball"),
        Err(Failure::Cost),
        "10 can't pay 20"
    );
    app.tick();
    assert!(close(get::<Mana>(&app, rich), 10.0), "paid");
    assert!(close(get::<Mana>(&app, poor), 10.0));
}

#[test]
fn an_effects_abilities_go_with_it_however_it_goes() {
    let mut app = app(&[channel()]);
    let mage = actor(&mut app, []);
    app.world_mut().apply_effect(mage, EffectSpec::new("staff"));
    app.tick();
    let staff = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    let ability = activate(&mut app, mage, "channel").unwrap();
    assert_eq!(
        app.entity(ability).try_get::<GrantedBy>(),
        Some(&GrantedBy(staff))
    );
    app.entity_mut(staff).despawn();
    app.tick();
    assert!(!app.entity(ability).is_spawned(), "taken away");
}

#[test]
fn an_active_ability_despawned_takes_back_its_tags() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    assert!(owns(&app, mage, "state.casting"));
    app.entity_mut(ability).despawn();
    app.tick();
    assert!(!owns(&app, mage, "state.casting"));
}

#[test]
fn a_new_registry_gives_granted_abilities_their_new_definitions() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    let world = app.world_mut();
    let effects = world.resource::<EffectRegistry>().clone();
    let free = fireball().commit(Commit::Manual);
    let registry = AbilityRegistry::check(
        &[free, channel()],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: world.resource::<Behaviours>(),
        },
    )
    .unwrap();
    world.insert_resource(registry);
    app.tick();
    assert_eq!(
        app.entity(ability).get::<GrantedAbility>().def().commit,
        Commit::Manual
    );
}

#[test]
fn an_ability_activates_with_its_input_and_ends_without_it() {
    let mut app = app(&[shot()]);
    let mage = caster(&mut app, &["shot"]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, "shot", Aim(4.0))
        .unwrap();
    app.entity(shooting)
        .assert_has::<Shoot>()
        .assert_eq(Aim(4.0));
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 10.0), "paid");
    app.world_mut().end_ability(shooting);
    app.entity(shooting)
        .assert_lacks::<Shoot>()
        .assert_lacks::<Aim>();
}

#[test]
fn input_has_to_be_given_for_the_abilitys_behaviour() {
    let mut app = app(&[shot(), fireball()]);
    app.watch_triggered::<AbilityFailed>();
    let mage = caster(&mut app, &["shot", "fireball"]);
    let world = app.world_mut();
    assert_eq!(
        world.activate_ability(mage, "shot"),
        Err(Failure::Input),
        "none given"
    );
    assert_eq!(
        world.activate_ability_with::<Shoot>(mage, "fireball", Aim(1.0)),
        Err(Failure::Input),
        "for another behaviour than the ability's"
    );
    assert_eq!(app.read_triggered::<AbilityFailed>().len(), 2, "each told");
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 30.0), "nothing paid");
}

#[test]
fn input_that_fails_its_check_rejects_the_activation_before_anything_is_paid() {
    let mut app = app(&[shot()]);
    let mage = caster(&mut app, &["shot"]);
    assert_eq!(
        app.world_mut()
            .activate_ability_with::<Shoot>(mage, "shot", Aim(12.0)),
        Err(Failure::Input)
    );
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 30.0));
    app.query::<(), With<Shoot>>().assert_len(0);
}

#[test]
fn new_input_replaces_the_old_and_input_that_fails_cancels_the_ability() {
    let mut app = app(&[shot()]);
    app.watch_triggered::<AbilityEnded>();
    app.watch_triggered::<AbilityFailed>();
    let mage = caster(&mut app, &["shot"]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, "shot", Aim(4.0))
        .unwrap();
    app.world_mut()
        .set_input::<Shoot>(shooting, Aim(-6.0))
        .unwrap();
    app.entity(shooting).assert_eq(Aim(-6.0));
    assert_eq!(
        app.world_mut().set_input::<Shoot>(shooting, Aim(30.0)),
        Err(Failure::Input)
    );
    app.entity(shooting)
        .assert_lacks::<Shoot>()
        .assert_lacks::<Aim>();
    app.assert_triggered::<AbilityEnded>(|ended| ended.cancelled);
    app.assert_triggered::<AbilityFailed>(|failed| failed.reason == Failure::Input);
    assert_eq!(
        app.world_mut().set_input::<Shoot>(shooting, Aim(1.0)),
        Err(Failure::Input),
        "no longer active"
    );
}

#[test]
fn an_ability_whose_behaviour_takes_input_cant_be_triggered() {
    let app = app(&[]);
    let triggered =
        Ability::new("snap_shot", "shoot").trigger(Trigger::Event(Id::new("event.parried")));
    let effects = app.resource::<EffectRegistry>().clone();
    let errors = AbilityRegistry::check(
        &[triggered],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: app.resource::<Behaviours>(),
        },
    )
    .unwrap_err()
    .to_string();
    assert!(
        errors.contains("triggers on an ability whose behaviour takes"),
        "{errors}"
    );
}

#[test]
fn input_for_another_behaviour_than_the_abilitys_cancels_it() {
    let mut app = app(&[shot()]);
    let mage = caster(&mut app, &["shot"]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, "shot", Aim(4.0))
        .unwrap();
    assert_eq!(
        app.world_mut().set_input::<Cast>(shooting, NoInput),
        Err(Failure::Input)
    );
    app.entity(shooting).assert_lacks::<Shoot>();
}

/// A cast a stun interrupts, and a rage that lasts only while raging.
fn interruptible() -> [Ability; 2] {
    [
        Ability::new("cast", "cast")
            .tag("ability.spell")
            .cancel_if(Condition::owner_has("state.stunned")),
        Ability::new("rage", "cast")
            .tag("ability.channel")
            .cancel_if(!Condition::owner_has("state.raging")),
    ]
}

#[test]
fn an_active_ability_is_interrupted_as_its_cancel_if_comes_to_hold() {
    let mut app = app(&interruptible());
    app.watch_triggered::<AbilityEnded>();
    let mage = caster(&mut app, &["cast", "rage"]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, "cast").unwrap();
    let rage = activate(&mut app, mage, "rage").unwrap();

    // Stunned: the cast is interrupted, the rage goes on.
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    app.entity(cast).assert_lacks::<ActiveAbility>();
    app.entity(rage).assert_has::<ActiveAbility>();
    app.assert_triggered::<AbilityEnded>(|ended| ended.ability == cast && ended.cancelled);

    // No longer raging: the rage is.
    own(&mut app, mage, "state.raging", false);
    app.tick();
    app.entity(rage).assert_lacks::<ActiveAbility>();
}

#[test]
fn an_ability_whose_cancel_if_holds_cant_activate() {
    let mut app = app(&interruptible());
    let mage = caster(&mut app, &["cast"]);
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    assert_eq!(activate(&mut app, mage, "cast"), Err(Failure::Interrupted));
}

#[test]
fn an_actors_abilities_are_cancelled_by_tag_or_all_at_once() {
    let mut app = app(&interruptible());
    let mage = caster(&mut app, &["cast", "rage"]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, "cast").unwrap();
    let rage = activate(&mut app, mage, "rage").unwrap();
    let spells: Tags = [Id::new("ability.spell")].into_iter().collect();
    assert_eq!(app.world_mut().cancel_abilities(mage, &spells), [cast]);
    app.entity(rage).assert_has::<ActiveAbility>();
    let cast = activate(&mut app, mage, "cast").unwrap();
    let mut all = app.world_mut().cancel_all_abilities(mage);
    all.sort();
    let mut both = vec![cast, rage];
    both.sort();
    assert_eq!(all, both);
}

#[test]
fn cost_and_cooldown_commit_apart_each_once() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();

    // The cooldown first: nothing paid yet.
    assert!(app.world_mut().commit_cooldown(ability));
    app.tick();
    assert!(owns(&app, mage, "cooldown.fireball"));
    assert!(close(get::<Mana>(&app, mage), 30.0), "not paid yet");
    let granted = app.entity(ability).get::<GrantedAbility>();
    assert!(granted.is_cooldown_started() && !granted.is_cost_paid());

    // Committing it all now pays the cost alone.
    assert!(app.world_mut().commit_ability(ability));
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 10.0));
    assert!(app.entity(ability).get::<GrantedAbility>().is_committed());
    assert!(app.world_mut().commit_cost(ability), "once per activation");
    app.tick();
    assert!(close(get::<Mana>(&app, mage), 10.0));
}

#[test]
fn a_cost_committed_alone_that_cant_be_paid_cancels() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    app.world_mut()
        .get_mut::<Base<Mana>>(mage)
        .unwrap()
        .set(5.0);
    app.tick();
    assert!(!app.world_mut().commit_cost(ability));
    app.entity(ability).assert_lacks::<ActiveAbility>();
    assert!(!owns(&app, mage, "cooldown.fireball"), "nor cooling down");
}

#[test]
fn an_ability_is_asked_about_with_nothing_done() {
    use bevy::ecs::system::RunSystemOnce;

    let mut app = app(&[fireball()]);
    let mage = actor(&mut app, []);
    let ability = app
        .world_mut()
        .grant_ability(mage, "fireball", 1.0)
        .unwrap();
    app.tick();
    let asked = |app: &mut TestApp| {
        app.world_mut()
            .run_system_once(move |reader: AbilityReader| {
                (
                    reader.can_activate(ability),
                    reader.can_afford(ability),
                    reader.cooldown_remaining(ability),
                )
            })
            .unwrap()
    };
    assert_eq!(asked(&mut app), (Ok(()), true, None));
    assert!(close(get::<Mana>(&app, mage), 30.0), "nothing paid asking");

    // Cast, ticked a step, ended, and a quarter second on: 10 mana left.
    assert_eq!(activate(&mut app, mage, "fireball"), Ok(ability));
    app.world_mut().end_ability(ability);
    app.secs(0.25);
    let (can, affords, remaining) = asked(&mut app);
    assert_eq!(can, Err(Failure::Cooldown));
    assert!(!affords);
    assert!(
        remaining.is_some_and(|left| close(left, 0.75 - 1.0 / 64.0)),
        "{remaining:?}"
    );
    assert_eq!(app.world().can_activate(ability), can, "as the world says");

    app.secs(1.0);
    assert_eq!(asked(&mut app), (Err(Failure::Cost), false, None));
}

#[test]
fn an_ability_committing_as_it_activates_pays_before_it_starts() {
    let mut app = app(&[Ability::new("witness", "witness").cost("fireball.cost")]);
    let mage = caster(&mut app, &["witness"]);
    activate(&mut app, mage, "witness").unwrap();
    let Witnessed(mana) = app.world().resource::<Witnessed>();
    assert!(
        mana.is_some_and(|mana| close(mana, 10.0)),
        "paid as its behaviour started: {mana:?}"
    );
}

#[test]
fn an_ability_without_a_cooldown_is_committed_once_it_pays() {
    let mut app = app(&[Ability::new("witness", "witness")
        .cost("fireball.cost")
        .commit(Commit::Manual)]);
    let mage = caster(&mut app, &["witness"]);
    let ability = activate(&mut app, mage, "witness").unwrap();
    let granted = app.entity(ability).get::<GrantedAbility>();
    assert!(granted.is_cooldown_started(), "nothing to start");
    assert!(!granted.is_committed());
    assert!(app.world_mut().commit_cost(ability));
    assert!(app.entity(ability).get::<GrantedAbility>().is_committed());
}

#[test]
fn an_ability_its_own_grants_interrupt_ends_as_it_starts() {
    let reckless = Ability::new("reckless", "cast")
        .grant("state.stunned")
        .cancel_if(Condition::owner_has("state.stunned"));
    let mut app = app(&[reckless]);
    let mage = caster(&mut app, &["reckless"]);
    let ability = activate(&mut app, mage, "reckless").unwrap();
    app.entity(ability).assert_lacks::<ActiveAbility>();
    assert!(!owns(&app, mage, "state.stunned"), "its grants taken back");
}

#[test]
fn a_new_definition_that_interrupts_an_active_ability_cancels_it() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = activate(&mut app, mage, "fireball").unwrap();
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    app.entity(ability).assert_has::<ActiveAbility>();

    let world = app.world_mut();
    let effects = world.resource::<EffectRegistry>().clone();
    let registry = AbilityRegistry::check(
        &[
            fireball().cancel_if(Condition::owner_has("state.stunned")),
            channel(),
        ],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: world.resource::<Behaviours>(),
        },
    )
    .unwrap();
    world.insert_resource(registry);
    app.tick();
    app.entity(ability).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_cooldown_owned_without_an_effect_has_no_end() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &["fireball"]);
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    own(&mut app, mage, "cooldown.fireball", true);
    app.tick();
    assert_eq!(app.world().can_activate(ability), Err(Failure::Cooldown));
    assert_eq!(app.world().cooldown_remaining(ability), Some(f32::INFINITY));
}
