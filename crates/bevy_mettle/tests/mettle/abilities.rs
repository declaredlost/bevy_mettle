//! Abilities: granted, activated, paid for, cooled down, blocked, cancelled,
//! triggered, committed by hand, and granted by effects.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::ability::data::NetExecution;
use bevy_mettle::core::Error;
use bevy_mettle::prelude::*;

use crate::common::{Mana, actor, attributes, close, get, own};

/// The behaviour every ability here is run by.
#[derive(Component, Default, Behaviour)]
struct Cast;

/// The ability `cast`, run by [`Cast`].
#[derive(Ability)]
#[ability(name = "cast")]
struct CastSpell;

/// A behaviour that notes its caster's mana as it starts ([`witness`]).
#[derive(Component, Default, Behaviour)]
struct Witness;

/// The ability `witness`, run by [`Witness`].
#[derive(Ability)]
#[ability(name = "witness")]
struct WitnessMana;

#[derive(Ability)]
struct Fireball;

#[derive(Ability)]
struct Channel;

#[derive(Ability)]
struct Interrupt;

#[derive(Ability)]
struct Spell;

#[derive(Ability)]
struct Riposte;

#[derive(Ability)]
struct Rage;

#[derive(Ability)]
struct Shot;

#[derive(Ability)]
struct Surge;

#[derive(Ability)]
struct Reckless;

/// An ability content never defines.
#[derive(Ability)]
struct Missing;

/// The effect granting `channel` while it lasts.
#[derive(Effect)]
struct Staff;

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

fn shot() -> AbilityData {
    AbilityData::new(Key::of::<Shot>(), Key::of::<Shoot>()).cost("fireball.cost")
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

fn tags() -> TagRegistry {
    TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap()
}

fn effects() -> Vec<EffectData> {
    vec![
        EffectData::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
        EffectData::lasting("fireball.cooldown", 1.0).grant("cooldown.fireball"),
        EffectData::infinite(Key::of::<Staff>()),
        EffectData::instant("surge.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -30.0))
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -30.0)),
    ]
}

fn fireball() -> AbilityData {
    AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>())
        .tag("ability.spell.fire")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .grant("state.casting")
        .blocked_by("state.stunned")
}

fn channel() -> AbilityData {
    AbilityData::new(Key::of::<Channel>(), Key::of::<Cast>())
        .tag("ability.channel")
        .blocks("ability.spell")
}

/// Adds the abilities code names here that `abilities` define: those alone,
/// as content without one that code names fails its check.
fn name_abilities(app: &mut App, abilities: &[AbilityData]) {
    fn add<A: Ability>(app: &mut App, abilities: &[AbilityData]) {
        if abilities
            .iter()
            .any(|ability| ability.name.as_str() == A::NAME)
        {
            app.add_ability::<A>();
        }
    }

    add::<CastSpell>(app, abilities);
    add::<WitnessMana>(app, abilities);
    add::<Fireball>(app, abilities);
    add::<Channel>(app, abilities);
    add::<Interrupt>(app, abilities);
    add::<Spell>(app, abilities);
    add::<Riposte>(app, abilities);
    add::<Rage>(app, abilities);
    add::<Shot>(app, abilities);
    add::<Surge>(app, abilities);
    add::<Reckless>(app, abilities);
}

/// What effects are checked against: what `world` has.
fn effect_checks(world: &World) -> EffectChecks<'_> {
    EffectChecks {
        tags: world.resource(),
        curves: world.resource(),
        attributes: world.resource(),
        executions: world.resource(),
        calculations: world.resource(),
        routes: world.resource(),
    }
}

/// What abilities are checked against: what `world` has.
fn ability_checks(world: &World) -> AbilityChecks<'_> {
    AbilityChecks {
        tags: world.resource(),
        effects: world.resource(),
        behaviours: world.resource(),
    }
}

/// `world`'s ability registry, rechecked with `abilities` and the staff's
/// grant: what a reload does.
fn reload(world: &mut World, abilities: &[AbilityData]) {
    let registry = world
        .resource::<AbilityRegistry>()
        .recheck(
            abilities,
            &[Grant::new(Key::of::<Staff>(), [Key::of::<Channel>()])],
            ability_checks(world),
        )
        .unwrap();
    world.insert_resource(registry);
}

/// An app with abilities `abilities` (and `channel`, which the staff grants),
/// and the effects they name.
fn app(abilities: &[AbilityData]) -> TestApp {
    let mut abilities = abilities.to_vec();
    if !abilities
        .iter()
        .any(|ability| ability.name.as_str() == "channel")
    {
        abilities.push(channel());
    }
    TestApp::builder()
        .with(attributes)
        .plugins((EffectPlugin, AbilityPlugin))
        .with(|app| {
            app.add_behaviour::<Cast>()
                .add_behaviour::<Shoot>()
                .add_behaviour::<Witness>()
                .add_observer(witness)
                .check_input::<Aim, _>(in_reach)
                .add_effect::<Staff>()
                .insert_resource(tags())
                .init_resource::<CurveRegistry>();
            name_abilities(app, &abilities);
            let world = app.world_mut();
            let effects = world
                .resource::<EffectRegistry>()
                .recheck(&effects(), effect_checks(world))
                .unwrap();
            world.insert_resource(effects);
            reload(world, &abilities);
        })
        .build()
}

/// `actor`, granted every one of `abilities`.
fn caster(app: &mut TestApp, abilities: &[&dyn IntoAbilityId]) -> Entity {
    let actor = actor(app, []);
    for ability in abilities {
        let id = ability
            .ability_id(app.resource::<AbilityRegistry>())
            .expect("an ability the app has");
        app.world_mut()
            .grant_ability(actor, AbilitySpec::from_id(id))
            .expect("granted");
    }
    actor
}

fn activate(
    app: &mut TestApp,
    actor: Entity,
    ability: impl IntoAbilityId,
) -> Result<Entity, Failure> {
    let activated = app.world_mut().activate_ability(actor, ability);
    app.tick();
    activated.map(Activated::ability)
}

fn owns(app: &TestApp, actor: Entity, tag: &'static str) -> bool {
    app.entity(actor).get::<OwnedTags>().contains(tag)
}

#[test]
fn activating_runs_the_behaviour_pays_and_cools_down() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
    assert!(app.entity(ability).has::<Cast>(), "its behaviour runs");
    assert_eq!(app.entity(ability).get::<ActiveAbility>().actor, mage);
    assert!(close(get::<Mana>(&app, mage), 10.0), "paid");
    assert!(owns(&app, mage, "state.casting"));
    app.world_mut().end_ability(ability);
    app.tick();
    app.entity(ability).assert_lacks::<Cast>();
    assert!(!owns(&app, mage, "state.casting"));
    assert_eq!(activate(&mut app, mage, Fireball), Err(Failure::Cooldown));
    app.secs(1.0);
    assert_eq!(
        activate(&mut app, mage, Fireball),
        Err(Failure::Cost),
        "10 mana left"
    );
}

#[test]
fn an_abilitys_cost_and_cooldown_carry_it_and_its_caster() {
    let mut app = app(&[fireball()]);
    app.watch_triggered::<EffectExecuted>();
    let mage = actor(&mut app, []);
    app.world_mut()
        .grant_ability(mage, Fireball.level(2.0))
        .expect("granted");
    activate(&mut app, mage, Fireball).unwrap();
    let applied_by = Some(AbilityRef {
        id: app
            .resource::<AbilityRegistry>()
            .id_of::<Fireball>()
            .unwrap(),
        level: 2.0,
    });
    let paid = app.read_triggered::<EffectExecuted>();
    let [cost] = &paid[..] else {
        panic!("the cost, once: {paid:?}");
    };
    assert_eq!(cost.context.ability, applied_by, "the cost");
    assert_eq!(cost.context.source, Some(mage));
    let effects: Vec<Entity> = app.entity(mage).get::<ActiveEffects>().iter().collect();
    let [cooldown] = effects[..] else {
        panic!("the cooldown alone lasts: {effects:?}");
    };
    let cooldown = app.entity(cooldown).get::<ActiveEffect>().spec().clone();
    assert_eq!(cooldown.context.ability, applied_by, "the cooldown");
    assert_eq!(cooldown.context.source, Some(mage));
    assert!(close(cooldown.level, 2.0), "at the ability's level");
}

#[test]
fn an_ability_is_blocked_by_its_owners_tags_and_active_abilities() {
    let mut app = app(&[fireball(), channel()]);
    let mage = caster(&mut app, &[&Fireball, &Channel]);
    own(&mut app, mage, "state.stunned", true);
    assert_eq!(activate(&mut app, mage, Fireball), Err(Failure::Blocked));
    own(&mut app, mage, "state.stunned", false);
    activate(&mut app, mage, Channel).unwrap();
    assert_eq!(
        activate(&mut app, mage, Fireball),
        Err(Failure::Blocked),
        "channelling"
    );
    assert_eq!(activate(&mut app, mage, Channel), Err(Failure::Active));
}

#[test]
fn activating_cancels_what_it_cancels() {
    let interrupt =
        AbilityData::new(Key::of::<Interrupt>(), Key::of::<Cast>()).cancels("ability.channel");
    let mut app = app(&[channel(), interrupt]);
    let mage = caster(&mut app, &[&Channel, &Interrupt]);
    let channelling = activate(&mut app, mage, Channel).unwrap();
    activate(&mut app, mage, Interrupt).unwrap();
    app.entity(channelling).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_requirement_has_to_hold() {
    let spell = AbilityData::new(Key::of::<Spell>(), Key::of::<Cast>())
        .require(!Condition::owner_has("state.silenced"));
    let mut app = app(&[spell]);
    let mage = caster(&mut app, &[&Spell]);
    own(&mut app, mage, "state.silenced", true);
    assert_eq!(activate(&mut app, mage, Spell), Err(Failure::Requirement));
    assert_eq!(activate(&mut app, mage, Missing), Err(Failure::NotGranted));
}

#[test]
fn a_manual_ability_pays_when_its_behaviour_commits() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
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
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
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
    let riposte = AbilityData::new(Key::of::<Riposte>(), Key::of::<Cast>())
        .trigger(Trigger::Event(TagKey::named("event")));
    let rage = AbilityData::new(Key::of::<Rage>(), Key::of::<Cast>())
        .trigger(Trigger::TagPresent(TagKey::named("state.raging")));
    let mut app = app(&[riposte, rage]);
    let knight = caster(&mut app, &[&Riposte, &Rage]);
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
    let mage = caster(&mut app, &[&Fireball, &Channel]);
    app.world_mut()
        .commands()
        .entity(mage)
        .activate_tagged([TagKey::named("ability.spell")].into_iter().collect());
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
    app.world_mut().apply_effect(mage, Staff);
    app.tick();
    let staff = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    activate(&mut app, mage, Channel).unwrap();
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
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
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
fn an_ability_left_out_is_as_new() {
    use serde_test::{Token, assert_de_tokens};

    assert_de_tokens(
        &AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>()),
        &[
            Token::Struct {
                name: "AbilityData",
                len: 2,
            },
            Token::Str("name"),
            Token::Str("fireball"),
            Token::Str("behaviour"),
            Token::Str("cast"),
            Token::StructEnd,
        ],
    );
}

#[test]
fn an_abilitys_optional_fields_read_as_serde_reads_them() {
    use serde_test::{Token, assert_de_tokens};

    assert_de_tokens(
        &AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>()).cost("fireball.cost"),
        &[
            Token::Struct {
                name: "AbilityData",
                len: 4,
            },
            Token::Str("name"),
            Token::Str("fireball"),
            Token::Str("behaviour"),
            Token::Str("cast"),
            Token::Str("cost"),
            Token::Some,
            Token::Str("fireball.cost"),
            Token::Str("cooldown"),
            Token::None,
            Token::StructEnd,
        ],
    );
}

#[test]
fn an_ability_with_everything_checks() {
    let def = AbilityData::new(Key::of::<Fireball>(), Key::of::<Cast>())
        .tag("ability.spell.fire")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .require(!Condition::owner_has("state.silenced"))
        .blocked_by("state.stunned")
        .trigger(Trigger::Event(TagKey::named("event.parried")))
        .commit(Commit::Manual);
    let app = app(&[def]);
    assert!(
        app.resource::<AbilityRegistry>()
            .id_of::<Fireball>()
            .is_some()
    );
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    let mut app = TestApp::builder()
        .with(attributes)
        .plugins((EffectPlugin, AbilityPlugin))
        .build();
    let world = app.world_mut();
    let attributes = world.resource::<AttributeRegistry>().clone();
    let effects = EffectRegistry::check(
        &effects(),
        EffectChecks {
            tags: &tags(),
            curves: &CurveRegistry::default(),
            attributes: &attributes,
            executions: &ExecutionRegistry::default(),
            calculations: &CalculationRegistry::default(),
            routes: &RouteRegistry::default(),
        },
    )
    .unwrap();
    let bad = AbilityData::new("bad", "nowhere")
        .tag("ability.unknown")
        .cost("fireball.cooldown")
        .cooldown("missing");
    let grants = [
        Grant::new(Key::of::<Staff>(), [Key::of::<Channel>()]),
        Grant::new("staf", [Key::of::<Channel>()]),
        Grant::new("fireball.cost", ["bad"]),
    ];
    let errors = AbilityRegistry::check(
        &[bad],
        &grants,
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: world.resource::<BehaviourRegistry>(),
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
    let surge = AbilityData::new(Key::of::<Surge>(), Key::of::<Cast>()).cost("surge.cost");
    let mut app = app(&[surge]);
    let mage = caster(&mut app, &[&Surge]);
    app.world_mut()
        .get_mut::<Base<Mana>>(mage)
        .unwrap()
        .set(50.0);
    app.tick();
    assert_eq!(
        activate(&mut app, mage, Surge),
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
        app.world_mut().grant_ability(mage, Fireball).unwrap();
    }
    // Nothing worked out yet: no tick since they were spawned.
    assert!(app.world_mut().activate_ability(rich, Fireball).is_ok());
    assert_eq!(
        app.world_mut().activate_ability(poor, Fireball),
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
    app.world_mut().apply_effect(mage, Staff);
    app.tick();
    let staff = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    let ability = activate(&mut app, mage, Channel).unwrap();
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
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
    assert!(owns(&app, mage, "state.casting"));
    app.entity_mut(ability).despawn();
    app.tick();
    assert!(!owns(&app, mage, "state.casting"));
}

#[test]
fn a_new_registry_gives_granted_abilities_their_new_definitions() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    reload(
        app.world_mut(),
        &[fireball().commit(Commit::Manual), channel()],
    );
    app.tick();
    assert_eq!(
        app.entity(ability).get::<GrantedAbility>().def().commit,
        Commit::Manual
    );
}

#[test]
fn an_ability_activates_with_its_input_and_ends_without_it() {
    let mut app = app(&[shot()]);
    let mage = caster(&mut app, &[&Shot]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, Shot, Aim(4.0))
        .unwrap()
        .ability();
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
    let mage = caster(&mut app, &[&Shot, &Fireball]);
    let world = app.world_mut();
    assert_eq!(
        world.activate_ability(mage, Shot),
        Err(Failure::Input),
        "none given"
    );
    assert_eq!(
        world.activate_ability_with::<Shoot>(mage, Fireball, Aim(1.0)),
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
    let mage = caster(&mut app, &[&Shot]);
    assert_eq!(
        app.world_mut()
            .activate_ability_with::<Shoot>(mage, Shot, Aim(12.0)),
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
    let mage = caster(&mut app, &[&Shot]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, Shot, Aim(4.0))
        .unwrap()
        .ability();
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
    let triggered = AbilityData::new("snap_shot", Key::of::<Shoot>())
        .trigger(Trigger::Event(TagKey::named("event.parried")));
    let effects = app.resource::<EffectRegistry>().clone();
    let errors = AbilityRegistry::check(
        &[triggered],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: app.resource::<BehaviourRegistry>(),
        },
    )
    .unwrap_err()
    .to_string();
    assert!(
        errors.contains("triggers on an ability whose behaviour takes"),
        "{errors}"
    );
}

/// What `abilities` fail their check with, if anything: checked against
/// `app`'s effects and behaviours.
fn check(app: &TestApp, abilities: &[AbilityData]) -> Result<(), String> {
    AbilityRegistry::check(
        abilities,
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: app.resource::<EffectRegistry>(),
            behaviours: app.resource::<BehaviourRegistry>(),
        },
    )
    .map(drop)
    .map_err(|errors| errors.to_string())
}

#[test]
fn an_ability_run_locally_only_cant_have_a_cost_or_a_cooldown() {
    let app = app(&[]);
    for local in [
        AbilityData::new("zoom", Key::of::<Cast>()).cost("fireball.cost"),
        AbilityData::new("zoom", Key::of::<Cast>()).cooldown("fireball.cooldown"),
    ] {
        let errors = check(&app, &[local.net(NetExecution::LocalOnly)]).unwrap_err();
        assert!(
            errors.contains("a cost or a cooldown on an ability run locally only"),
            "{errors}"
        );
    }
}

#[test]
fn an_ability_run_locally_only_without_a_cost_checks() {
    let app = app(&[]);
    let zoom = AbilityData::new("zoom", Key::of::<Cast>()).net(NetExecution::LocalOnly);
    assert_eq!(check(&app, &[zoom]), Ok(()));
    // Run otherwise, the same cost is fine.
    for net in [
        NetExecution::Predicted,
        NetExecution::ServerOnly,
        NetExecution::ServerInitiated,
    ] {
        let paid = AbilityData::new("zoom", Key::of::<Cast>())
            .cost("fireball.cost")
            .net(net);
        assert_eq!(check(&app, &[paid]), Ok(()), "{net:?}");
    }
}

#[test]
fn input_for_another_behaviour_than_the_abilitys_cancels_it() {
    let mut app = app(&[shot()]);
    let mage = caster(&mut app, &[&Shot]);
    let shooting = app
        .world_mut()
        .activate_ability_with::<Shoot>(mage, Shot, Aim(4.0))
        .unwrap()
        .ability();
    assert_eq!(
        app.world_mut().set_input::<Cast>(shooting, NoInput),
        Err(Failure::Input)
    );
    app.entity(shooting).assert_lacks::<Shoot>();
}

/// A cast a stun interrupts, and a rage that lasts only while raging.
fn interruptible() -> [AbilityData; 2] {
    [
        AbilityData::new(Key::of::<CastSpell>(), Key::of::<Cast>())
            .tag("ability.spell")
            .cancel_if(Condition::owner_has("state.stunned")),
        AbilityData::new(Key::of::<Rage>(), Key::of::<Cast>())
            .tag("ability.channel")
            .cancel_if(!Condition::owner_has("state.raging")),
    ]
}

#[test]
fn an_active_ability_is_interrupted_as_its_cancel_if_comes_to_hold() {
    let mut app = app(&interruptible());
    app.watch_triggered::<AbilityEnded>();
    let mage = caster(&mut app, &[&CastSpell, &Rage]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, CastSpell).unwrap();
    let rage = activate(&mut app, mage, Rage).unwrap();

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
    let mage = caster(&mut app, &[&CastSpell]);
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    assert_eq!(
        activate(&mut app, mage, CastSpell),
        Err(Failure::Interrupted)
    );
}

#[test]
fn an_actors_abilities_are_cancelled_by_tag_or_all_at_once() {
    let mut app = app(&interruptible());
    let mage = caster(&mut app, &[&CastSpell, &Rage]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, CastSpell).unwrap();
    let rage = activate(&mut app, mage, Rage).unwrap();
    let spells: Tags = [TagKey::named("ability.spell")].into_iter().collect();
    assert_eq!(app.world_mut().cancel_abilities(mage, &spells), [cast]);
    app.entity(rage).assert_has::<ActiveAbility>();
    let cast = activate(&mut app, mage, CastSpell).unwrap();
    let mut all = app.world_mut().cancel_all_abilities(mage);
    all.sort();
    let mut both = vec![cast, rage];
    both.sort();
    assert_eq!(all, both);
}

#[test]
fn cost_and_cooldown_commit_apart_each_once() {
    let mut app = app(&[fireball().commit(Commit::Manual)]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();

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
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
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
    let ability = app.world_mut().grant_ability(mage, Fireball).unwrap();
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
    assert_eq!(activate(&mut app, mage, Fireball), Ok(ability));
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
    let mut app = app(&[
        AbilityData::new(Key::of::<WitnessMana>(), Key::of::<Witness>()).cost("fireball.cost"),
    ]);
    let mage = caster(&mut app, &[&WitnessMana]);
    activate(&mut app, mage, WitnessMana).unwrap();
    let Witnessed(mana) = app.world().resource::<Witnessed>();
    assert!(
        mana.is_some_and(|mana| close(mana, 10.0)),
        "paid as its behaviour started: {mana:?}"
    );
}

#[test]
fn an_ability_without_a_cooldown_is_committed_once_it_pays() {
    let mut app = app(&[
        AbilityData::new(Key::of::<WitnessMana>(), Key::of::<Witness>())
            .cost("fireball.cost")
            .commit(Commit::Manual),
    ]);
    let mage = caster(&mut app, &[&WitnessMana]);
    let ability = activate(&mut app, mage, WitnessMana).unwrap();
    let granted = app.entity(ability).get::<GrantedAbility>();
    assert!(granted.is_cooldown_started(), "nothing to start");
    assert!(!granted.is_committed());
    assert!(app.world_mut().commit_cost(ability));
    assert!(app.entity(ability).get::<GrantedAbility>().is_committed());
}

#[test]
fn an_ability_its_own_grants_interrupt_ends_as_it_starts() {
    let reckless = AbilityData::new(Key::of::<Reckless>(), Key::of::<Cast>())
        .grant("state.stunned")
        .cancel_if(Condition::owner_has("state.stunned"));
    let mut app = app(&[reckless]);
    let mage = caster(&mut app, &[&Reckless]);
    let ability = activate(&mut app, mage, Reckless).unwrap();
    app.entity(ability).assert_lacks::<ActiveAbility>();
    assert!(!owns(&app, mage, "state.stunned"), "its grants taken back");
}

#[test]
fn a_new_definition_that_interrupts_an_active_ability_cancels_it() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    app.entity(ability).assert_has::<ActiveAbility>();

    reload(
        app.world_mut(),
        &[
            fireball().cancel_if(Condition::owner_has("state.stunned")),
            channel(),
        ],
    );
    app.tick();
    app.entity(ability).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_cooldown_owned_without_an_effect_has_no_end() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &[&Fireball]);
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    own(&mut app, mage, "cooldown.fireball", true);
    app.tick();
    assert_eq!(app.world().can_activate(ability), Err(Failure::Cooldown));
    assert_eq!(app.world().cooldown_remaining(ability), Some(f32::INFINITY));
}

#[test]
fn a_reload_keeps_every_number_and_granted_abilities_working() {
    let mut app = app(&[fireball()]);
    let mage = caster(&mut app, &[&Fireball]);
    app.world_mut().apply_effect(mage, Staff);
    app.tick();
    let effects_before = app.resource::<EffectRegistry>().clone();
    let cost = effects_before.id("fireball.cost").expect("the cost");
    let staff = effects_before.id_of::<Staff>().expect("the staff");
    let fireball_id = app
        .resource::<AbilityRegistry>()
        .id_of::<Fireball>()
        .expect("fireball");

    // The same content and a new effect, named before every other.
    let mut reloaded = effects();
    reloaded.push(
        EffectData::instant("blast").modifier(EffectModifier::new::<Mana>(Operation::Flat, -1.0)),
    );
    let world = app.world_mut();
    let registry = world
        .resource::<EffectRegistry>()
        .recheck(&reloaded, effect_checks(world))
        .unwrap();
    world.insert_resource(registry);
    reload(world, &[fireball(), channel()]);
    app.tick();

    let effects = app.resource::<EffectRegistry>();
    assert_eq!(effects.id("fireball.cost"), Some(cost), "the cost's number");
    assert_eq!(effects.id_of::<Staff>(), Some(staff), "the staff's number");
    let blast = effects.id("blast").expect("the new effect");
    assert_eq!(
        effects.iter().last().map(|(id, _)| id),
        Some(blast),
        "numbered after the last"
    );
    assert_eq!(
        app.resource::<AbilityRegistry>().id_of::<Fireball>(),
        Some(fireball_id),
        "fireball's number"
    );
    let worn = app
        .entity(mage)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    assert_eq!(
        app.entity(worn).get::<ActiveEffect>().id(),
        staff,
        "the active staff still the staff"
    );

    activate(&mut app, mage, Fireball).unwrap();
    assert!(close(get::<Mana>(&app, mage), 10.0), "paid");
    assert!(owns(&app, mage, "state.casting"));
}

#[test]
fn code_naming_an_ability_content_lacks_fails_the_check() {
    let mut app = App::new();
    app.add_ability::<Missing>();
    let errors = app
        .world()
        .resource::<AbilityRegistry>()
        .recheck(
            &[fireball()],
            &[],
            AbilityChecks {
                tags: &tags(),
                effects: &EffectRegistry::default(),
                behaviours: &BehaviourRegistry::default(),
            },
        )
        .unwrap_err();
    let missing = Error::NotInContent {
        kind: "ability",
        name: "missing".to_owned(),
    };
    assert!(errors.0.contains(&missing), "{errors}");
    assert!(
        errors
            .to_string()
            .contains("code names ability `missing`, which content doesn't define"),
        "{errors}"
    );
}

#[test]
fn an_ability_is_granted_at_its_specs_level() {
    let mut app = app(&[fireball()]);
    let mage = actor(&mut app, []);
    let level =
        |app: &TestApp, ability: Entity| app.entity(ability).get::<GrantedAbility>().level();

    let ability = app
        .world_mut()
        .grant_ability(mage, Fireball.level(2.0))
        .unwrap();
    assert!(close(level(&app, ability), 2.0));
    let plain = actor(&mut app, []);
    let ability = app.world_mut().grant_ability(plain, Fireball).unwrap();
    assert!(close(level(&app, ability), 1.0), "1 unless it says");

    // By number, and by command.
    let id = app
        .resource::<AbilityRegistry>()
        .id_of::<Fireball>()
        .unwrap();
    let other = actor(&mut app, []);
    let by_number = app
        .world_mut()
        .grant_ability(other, AbilitySpec::from_id(id).level(3.0))
        .unwrap();
    assert!(close(level(&app, by_number), 3.0));
    let third = actor(&mut app, []);
    app.world_mut()
        .commands()
        .entity(third)
        .grant_ability(Fireball.level(4.0));
    app.tick();
    let granted = app.entity(third).get::<Abilities>().iter().next().unwrap();
    assert!(close(level(&app, granted), 4.0));
}
