//! Abilities: granted, activated, paid for, cooled down, blocked, cancelled,
//! triggered, committed by hand, and granted by effects.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::MettleContentPlugin;
use bevy_mettle::ability::AbilityScope;
use bevy_mettle::ability::behaviour::BehaviourRegistry;
use bevy_mettle::ability::data::NetExecution;
use bevy_mettle::core::Error;
use bevy_mettle::effect::EffectScope;
use bevy_mettle::effect::code::{CalculationRegistry, ExecutionRegistry, RouteRegistry};
use bevy_mettle::effect::context::ContextDataRegistry;
use bevy_mettle::prelude::*;

use crate::common::{Mana, actor, attributes, close, get, own};

/// The behaviour every ability here is run by.
#[derive(Component, Default, Behaviour)]
struct Cast;

/// A behaviour that notes its caster's mana as it starts ([`witness`]).
#[derive(Component, Default, Behaviour)]
struct Witness;

/// Where a shot is aimed: how far along.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Aim(f32);

/// A shot being fired: aimed.
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct Shoot;

/// A fire spell: paid for, cooled down, held off by a stun.
#[derive(Ability)]
#[ability(define = fireball)]
struct Fireball;

/// [`Fireball`], paid for and cooled down when its behaviour commits.
#[derive(Ability)]
#[ability(define = charged_fireball)]
struct ChargedFireball;

/// A channel, blocking spells, granted by the staff while it lasts.
#[derive(Ability)]
#[ability(behaviour::<Cast>, tag = "ability.channel", blocks = "ability.spell")]
#[ability(granted_by::<Staff>)]
struct Channel;

/// Ends channels as it activates.
#[derive(Ability)]
#[ability(behaviour::<Cast>, ends = "ability.channel")]
struct Interrupt;

/// A spell a silence holds off.
#[derive(Ability)]
#[ability(behaviour::<Cast>, require = !Condition::owner_has("state.silenced"))]
struct Spell;

/// Activated by events.
#[derive(Ability)]
#[ability(behaviour::<Cast>, trigger = Trigger::Event(TagKey::named("event")))]
struct Riposte;

/// Active while its owner's raging.
#[derive(Ability)]
#[ability(behaviour::<Cast>, trigger = Trigger::TagPresent(TagKey::named("state.raging")))]
struct Rage;

/// A cast a stun interrupts.
#[derive(Ability)]
#[ability(name = "cast", behaviour::<Cast>, tag = "ability.spell")]
#[ability(end_if = Condition::owner_has("state.stunned"))]
struct CastSpell;

/// A frenzy that lasts only while raging.
#[derive(Ability)]
#[ability(behaviour::<Cast>, tag = "ability.channel")]
#[ability(end_if = !Condition::owner_has("state.raging"))]
struct Frenzy;

/// A shot, paid for.
#[derive(Ability)]
#[ability(behaviour::<Shoot>, cost = Cost::of::<Mana>(20.0))]
struct Shot;

/// Costs 30 mana, twice.
#[derive(Ability)]
#[ability(behaviour::<Cast>, cost = Cost::of::<Mana>(30.0), cost = Cost::of::<Mana>(30.0))]
struct Surge;

/// Stuns its own owner, which interrupts it.
#[derive(Ability)]
#[ability(behaviour::<Cast>, owner_tag = "state.stunned")]
#[ability(end_if = Condition::owner_has("state.stunned"))]
struct Reckless;

/// The ability `witness`, run by [`Witness`], paying as it activates.
#[derive(Ability)]
#[ability(name = "witness", behaviour::<Witness>, cost = Cost::of::<Mana>(20.0))]
struct WitnessMana;

/// [`WitnessMana`], paying when its behaviour commits.
#[derive(Ability)]
#[ability(behaviour::<Witness>, cost = Cost::of::<Mana>(20.0), commit = Commit::Manual)]
struct WitnessLater;

/// An ability with every key.
#[derive(Ability)]
#[ability(behaviour::<Cast>, tag = "ability.spell.fire", owner_tag = "state.casting")]
#[ability(cost = Cost::of::<Mana>(20.0), cooldown = 1.0, cooldown_tag = "cooldown")]
#[ability(require = Condition::all([
    !Condition::owner_has("state.silenced"),
    !Condition::owner_has("state.stunned"),
]))]
#[ability(end_if = Condition::owner_has("state.stunned"))]
#[ability(ends = "ability.channel", blocks = "ability.spell")]
#[ability(trigger = Trigger::Event(TagKey::named("event.parried")))]
#[ability(granted_by::<Staff>, commit = Commit::Manual, net = NetExecution::ServerOnly)]
struct Everything;

/// An ability the app never adds.
#[derive(Ability)]
#[ability(behaviour::<Cast>)]
struct Missing;

/// The effect granting `channel` while it lasts.
#[derive(Effect)]
#[effect(infinite)]
struct Staff;

/// The effect granting what content says while it lasts: `bolt`.
#[derive(Effect)]
#[effect(infinite)]
struct Tome;

fn fireball(ability: AbilityData) -> AbilityData {
    ability
        .behaviour::<Cast>()
        .tag("ability.spell.fire")
        .cost(Cost::of::<Mana>(20.0))
        .cooldown(1.0)
        .owner_tag("state.casting")
        .require(!Condition::owner_has("state.stunned"))
}

fn charged_fireball(ability: AbilityData) -> AbilityData {
    fireball(ability).commit(Commit::Manual)
}

/// The ability `bolt`, as content defines it: granted by the tome.
fn bolt() -> AbilityData {
    let mut bolt = AbilityData::new("bolt").behaviour::<Cast>();
    bolt.granted_by.push(Key::of::<Tome>());
    bolt
}

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

/// Whether `check`'s aim is within reach: 10 along, either way.
fn in_reach(In(check): In<InputCheck<Aim>>) -> bool {
    check.input.0.abs() <= 10.0
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
    "event",
    "event.parried",
];

/// The tags the tests' content adds.
fn tags() -> Vec<TagKey> {
    TAGS.iter().map(|&name| TagKey::named(name)).collect()
}

/// An app with the abilities and effects here, and content adding the tags.
fn app() -> TestApp {
    app_with(|_| {})
}

/// [`app`], with what `more` adds before it starts.
fn app_with(more: impl FnOnce(&mut App)) -> TestApp {
    TestApp::builder()
        .with(attributes)
        .plugins((EffectPlugin, AbilityPlugin, MettleContentPlugin))
        .with(|app| {
            app.add_behaviour::<Cast>()
                .add_behaviour::<Shoot>()
                .add_behaviour::<Witness>()
                .add_observer(witness)
                .check_input::<Aim, _>(in_reach)
                .add_effects::<(Staff, Tome)>()
                .add_abilities::<(
                    Fireball,
                    ChargedFireball,
                    Channel,
                    Interrupt,
                    Spell,
                    Riposte,
                    Rage,
                    CastSpell,
                    Frenzy,
                    Shot,
                    Surge,
                    Reckless,
                    WitnessMana,
                    WitnessLater,
                )>()
                .insert_resource(MettleContent {
                    tags: tags(),
                    ..default()
                });
        })
        .with(more)
        .build()
}

/// `app`'s content, changed by `change`: loaded as the next tick starts.
fn reload(app: &mut TestApp, change: impl FnOnce(&mut MettleContent)) {
    change(&mut app.world_mut().resource_mut::<MettleContent>());
    app.tick();
}

/// `actor`, granted every one of `abilities`.
fn caster(app: &mut TestApp, abilities: impl IntoIterator<Item = AbilitySpec>) -> Entity {
    let actor = actor(app, []);
    for ability in abilities {
        app.world_mut()
            .grant_ability(actor, ability)
            .expect("granted");
    }
    actor
}

fn activate(app: &mut TestApp, actor: Entity, ability: impl Ability) -> Result<Entity, Failure> {
    let activated = app.world_mut().activate_ability(actor, ability);
    app.tick();
    activated.map(Activated::ability)
}

fn owns(app: &TestApp, actor: Entity, tag: &'static str) -> bool {
    app.entity(actor).get::<OwnedTags>().contains(tag)
}

#[test]
fn activating_runs_the_behaviour_pays_and_cools_down() {
    let mut app = app();
    let mage = caster(&mut app, [Fireball.spec()]);
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
    let mut app = app();
    app.watch_triggered::<EffectExecuted>();
    let mage = actor(&mut app, []);
    app.world_mut()
        .grant_ability(mage, Fireball.level(2.0))
        .expect("granted");
    activate(&mut app, mage, Fireball).unwrap();
    let applied_by = Some(&AbilityRef {
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
    assert_eq!(cost.context.get::<AbilityRef>(), applied_by, "the cost");
    assert_eq!(cost.context.source, Some(mage));
    let effects: Vec<Entity> = app.entity(mage).get::<ActiveEffects>().iter().collect();
    let [cooldown] = effects[..] else {
        panic!("the cooldown alone lasts: {effects:?}");
    };
    let cooldown = app.entity(cooldown).get::<ActiveEffect>().spec().clone();
    assert_eq!(
        cooldown.context.get::<AbilityRef>(),
        applied_by,
        "the cooldown"
    );
    assert_eq!(cooldown.context.source, Some(mage));
    assert!(close(cooldown.level, 2.0), "at the ability's level");
}

#[test]
fn an_ability_is_blocked_by_active_abilities() {
    let mut app = app();
    let mage = caster(&mut app, [Fireball.spec(), Channel.spec()]);
    // What its owner owns is its requirement's to say.
    own(&mut app, mage, "state.stunned", true);
    assert_eq!(
        activate(&mut app, mage, Fireball),
        Err(Failure::Requirement)
    );
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
fn activating_ends_what_it_ends() {
    let mut app = app();
    let mage = caster(&mut app, [Channel.spec(), Interrupt.spec()]);
    let channelling = activate(&mut app, mage, Channel).unwrap();
    activate(&mut app, mage, Interrupt).unwrap();
    app.entity(channelling).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_requirement_has_to_hold() {
    let mut app = app();
    let mage = caster(&mut app, [Spell.spec()]);
    own(&mut app, mage, "state.silenced", true);
    assert_eq!(activate(&mut app, mage, Spell), Err(Failure::Requirement));
    assert_eq!(activate(&mut app, mage, Missing), Err(Failure::NotGranted));
}

#[test]
fn a_manual_ability_pays_when_its_behaviour_commits() {
    let mut app = app();
    let mage = caster(&mut app, [ChargedFireball.spec()]);
    let ability = activate(&mut app, mage, ChargedFireball).unwrap();
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
    let mut app = app();
    let mage = caster(&mut app, [ChargedFireball.spec()]);
    let ability = activate(&mut app, mage, ChargedFireball).unwrap();
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
    let mut app = app();
    let knight = caster(&mut app, [Riposte.spec(), Rage.spec()]);
    let foe = actor(&mut app, []);
    app.world_mut()
        .trigger(GameplayEvent::new(knight, "event.parried").source(foe));
    app.tick();
    let abilities: Vec<Entity> = app.entity(knight).get::<Abilities>().iter().collect();
    let active = app.entity(abilities[0]).get::<ActiveAbility>();
    assert_eq!(
        active.event.as_ref().unwrap().context.source,
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
    let mut app = app();
    let mage = caster(&mut app, [Fireball.spec(), Channel.spec()]);
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
    let mut app = app();
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
    app.world_mut().despawn(staff);
    app.tick();
    assert!(
        app.entity(mage)
            .try_get::<Abilities>()
            .is_none_or(|abilities| abilities.iter().count() == 0)
    );
}

#[test]
fn a_cooldown_applied_under_an_activation_can_be_taken_back() {
    let mut app = app();
    let mage = caster(&mut app, [Fireball.spec()]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
    let activation = app.entity(ability).get::<ActiveAbility>().activation;
    app.world_mut().cancel_ability(ability);
    app.world_mut()
        .commands()
        .entity(mage)
        .remove_effects_under(activation);
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
        &AbilityData::new(Key::of::<Fireball>()).behaviour::<Cast>(),
        &[
            Token::Struct {
                name: "AbilityData",
                len: 2,
            },
            Token::Str("name"),
            Token::Str("fireball"),
            Token::Str("behaviour"),
            Token::Some,
            Token::Str("cast"),
            Token::StructEnd,
        ],
    );
}

#[test]
fn an_abilitys_optional_fields_read_as_serde_reads_them() {
    use serde_test::{Token, assert_de_tokens};

    assert_de_tokens(
        &AbilityData::new(Key::of::<Fireball>())
            .behaviour::<Cast>()
            .cost(Cost::of::<Mana>(20.0)),
        &[
            Token::Struct {
                name: "AbilityData",
                len: 4,
            },
            Token::Str("name"),
            Token::Str("fireball"),
            Token::Str("behaviour"),
            Token::Some,
            Token::Str("cast"),
            Token::Str("cost"),
            Token::Seq { len: Some(1) },
            Token::Struct {
                name: "Cost",
                len: 2,
            },
            Token::Str("attribute"),
            Token::Str("mana"),
            Token::Str("amount"),
            Token::NewtypeVariant {
                name: "Magnitude",
                variant: "Value",
            },
            Token::F32(20.0),
            Token::StructEnd,
            Token::SeqEnd,
            Token::Str("cooldown"),
            Token::None,
            Token::StructEnd,
        ],
    );
}

#[test]
fn an_ability_with_everything_checks() {
    let app = app_with(|app| {
        app.add_ability::<Everything>();
    });
    assert!(
        app.resource::<AbilityRegistry>()
            .id_of::<Everything>()
            .is_some()
    );
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    let app = app();
    let tags = TagRegistry::build(tags()).unwrap();
    let effects = EffectRegistry::build(
        &[
            EffectData::new("staff").infinite(),
            EffectData::new("blast"),
        ],
        EffectScope {
            tags: &tags,
            curves: &CurveRegistry::default(),
            attributes: app.resource::<AttributeRegistry>(),
            executions: &ExecutionRegistry::default(),
            calculations: &CalculationRegistry::default(),
            routes: &RouteRegistry::default(),
            context: &ContextDataRegistry::default(),
        },
    )
    .unwrap();
    let mut bad = AbilityData::new("bad")
        .tag("ability.unknown")
        .cost(Cost::of::<Mana>(5.0));
    bad.behaviour = Some("nowhere".into());
    bad.granted_by = vec!["staff".into(), "staf".into(), "blast".into()];
    let idle = AbilityData::new("idle");
    let errors = AbilityRegistry::build(
        &[bad, idle],
        AbilityScope {
            tags: &tags,
            effects: &effects,
            behaviours: app.resource::<BehaviourRegistry>(),
        },
    )
    .unwrap_err()
    .to_string();
    for expected in [
        "there's no behaviour called `nowhere` (in `bad`)",
        "there's no tag called `ability.unknown` (in `bad`)",
        // Built alone, nothing made its cost an effect.
        "there's no effect called `bad.cost` (in `bad`)",
        "there's no effect called `staf` (in `bad`)",
        "granted by `blast`: it's instant, so it grants nothing for any while (in `bad`)",
        "an ability with no behaviour: nothing runs it (in `idle`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}

#[test]
fn a_cost_is_checked_change_after_change() {
    let mut app = app();
    let mage = caster(&mut app, [Surge.spec()]);
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
    let mut app = app();
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
    let mut app = app();
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
fn an_active_ability_despawned_is_cancelled() {
    let mut app = app();
    app.watch_triggered::<AbilityEnded>();
    let mage = caster(&mut app, [Fireball.spec()]);
    let ability = activate(&mut app, mage, Fireball).unwrap();
    assert!(owns(&app, mage, "state.casting"));
    app.world_mut().despawn(ability);
    // The world flushes as it despawns: ended at once.
    assert!(!owns(&app, mage, "state.casting"));
    app.assert_triggered::<AbilityEnded>(|ended| ended.ability == ability && ended.cancelled);
}

#[test]
fn a_new_registry_gives_granted_abilities_their_new_definitions() {
    let mut app = app();
    reload(&mut app, |content| content.abilities.push(bolt()));
    let mage = actor(&mut app, []);
    app.world_mut().apply_effect(mage, Tome);
    app.tick();
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    assert_eq!(
        app.entity(ability).get::<GrantedAbility>().def().commit,
        Commit::OnActivate
    );
    reload(&mut app, |content| {
        content.abilities = vec![bolt().commit(Commit::Manual)];
    });
    assert_eq!(
        app.entity(ability).get::<GrantedAbility>().def().commit,
        Commit::Manual
    );
}

#[test]
fn an_ability_activates_with_its_input_and_ends_without_it() {
    let mut app = app();
    let mage = caster(&mut app, [Shot.spec()]);
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
    let mut app = app();
    app.watch_triggered::<AbilityFailed>();
    let mage = caster(&mut app, [Shot.spec(), Fireball.spec()]);
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
    let mut app = app();
    let mage = caster(&mut app, [Shot.spec()]);
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
    let mut app = app();
    app.watch_triggered::<AbilityEnded>();
    app.watch_triggered::<AbilityFailed>();
    let mage = caster(&mut app, [Shot.spec()]);
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
    let app = app();
    let triggered = AbilityData::new("snap_shot")
        .behaviour::<Shoot>()
        .trigger(Trigger::Event(TagKey::named("event.parried")));
    let errors = check(&app, &[triggered]).unwrap_err();
    assert!(
        errors.contains("triggers on an ability whose behaviour takes"),
        "{errors}"
    );
}

/// What content adding `abilities` fails its check with, if anything:
/// checked against what `app` has, with the tags its content adds.
fn check(app: &TestApp, abilities: &[AbilityData]) -> Result<(), String> {
    MettleContent {
        abilities: abilities.to_vec(),
        ..app.resource::<MettleContent>().clone()
    }
    .check(app.world())
    .map_err(|errors| errors.to_string())
}

#[test]
fn an_ability_run_locally_only_cant_have_a_cost_or_a_cooldown() {
    let app = app();
    let zoom = || AbilityData::new("zoom").behaviour::<Cast>();
    for local in [zoom().cost(Cost::of::<Mana>(20.0)), zoom().cooldown(1.0)] {
        let errors = check(&app, &[local.net(NetExecution::LocalOnly)]).unwrap_err();
        assert!(
            errors.contains("a cost or a cooldown on an ability run locally only"),
            "{errors}"
        );
    }
}

#[test]
fn an_ability_run_locally_only_without_a_cost_checks() {
    let app = app();
    let zoom = || AbilityData::new("zoom").behaviour::<Cast>();
    assert_eq!(check(&app, &[zoom().net(NetExecution::LocalOnly)]), Ok(()));
    // Run otherwise, the same cost is fine.
    for net in [
        NetExecution::Predicted,
        NetExecution::ServerOnly,
        NetExecution::ServerInitiated,
    ] {
        let paid = zoom().cost(Cost::of::<Mana>(20.0)).net(net);
        assert_eq!(check(&app, &[paid]), Ok(()), "{net:?}");
    }
}

#[test]
fn input_for_another_behaviour_than_the_abilitys_cancels_it() {
    let mut app = app();
    let mage = caster(&mut app, [Shot.spec()]);
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

#[test]
fn an_active_ability_is_interrupted_as_its_end_if_comes_to_hold() {
    let mut app = app();
    app.watch_triggered::<AbilityEnded>();
    let mage = caster(&mut app, [CastSpell.spec(), Frenzy.spec()]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, CastSpell).unwrap();
    let rage = activate(&mut app, mage, Frenzy).unwrap();

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
fn an_ability_whose_end_if_holds_cant_activate() {
    let mut app = app();
    let mage = caster(&mut app, [CastSpell.spec()]);
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    assert_eq!(
        activate(&mut app, mage, CastSpell),
        Err(Failure::Interrupted)
    );
}

#[test]
fn an_actors_abilities_are_cancelled_by_tag_or_all_at_once() {
    let mut app = app();
    let mage = caster(&mut app, [CastSpell.spec(), Frenzy.spec()]);
    own(&mut app, mage, "state.raging", true);
    app.tick();
    let cast = activate(&mut app, mage, CastSpell).unwrap();
    let rage = activate(&mut app, mage, Frenzy).unwrap();
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
    let mut app = app();
    let mage = caster(&mut app, [ChargedFireball.spec()]);
    let ability = activate(&mut app, mage, ChargedFireball).unwrap();

    // The cooldown first: nothing paid yet.
    assert!(app.world_mut().commit_cooldown(ability));
    app.tick();
    assert!(owns(&app, mage, "cooldown.charged_fireball"));
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
    let mut app = app();
    let mage = caster(&mut app, [ChargedFireball.spec()]);
    let ability = activate(&mut app, mage, ChargedFireball).unwrap();
    app.world_mut()
        .get_mut::<Base<Mana>>(mage)
        .unwrap()
        .set(5.0);
    app.tick();
    assert!(!app.world_mut().commit_cost(ability));
    app.entity(ability).assert_lacks::<ActiveAbility>();
    assert!(
        !owns(&app, mage, "cooldown.charged_fireball"),
        "nor cooling down"
    );
}

#[test]
fn an_ability_is_asked_about_with_nothing_done() {
    use bevy::ecs::system::RunSystemOnce;

    let mut app = app();
    let mage = actor(&mut app, []);
    let ability = app.world_mut().grant_ability(mage, Fireball).unwrap();
    app.tick();
    let asked = |app: &mut TestApp| {
        app.world_mut()
            .run_system_once(move |world: &World| {
                (
                    world.can_activate(ability),
                    world.can_afford(ability),
                    world.cooldown_remaining(ability),
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
    let mut app = app();
    let mage = caster(&mut app, [WitnessMana.spec()]);
    activate(&mut app, mage, WitnessMana).unwrap();
    let Witnessed(mana) = app.world().resource::<Witnessed>();
    assert!(
        mana.is_some_and(|mana| close(mana, 10.0)),
        "paid as its behaviour started: {mana:?}"
    );
}

#[test]
fn an_ability_without_a_cooldown_is_committed_once_it_pays() {
    let mut app = app();
    let mage = caster(&mut app, [WitnessLater.spec()]);
    let ability = activate(&mut app, mage, WitnessLater).unwrap();
    let granted = app.entity(ability).get::<GrantedAbility>();
    assert!(granted.is_cooldown_started(), "nothing to start");
    assert!(!granted.is_committed());
    assert!(app.world_mut().commit_cost(ability));
    assert!(app.entity(ability).get::<GrantedAbility>().is_committed());
}

#[test]
fn an_ability_its_own_owner_tags_interrupt_ends_as_it_starts() {
    let mut app = app();
    let mage = caster(&mut app, [Reckless.spec()]);
    let ability = activate(&mut app, mage, Reckless).unwrap();
    app.entity(ability).assert_lacks::<ActiveAbility>();
    assert!(!owns(&app, mage, "state.stunned"), "its grants taken back");
}

#[test]
fn a_new_definition_that_interrupts_an_active_ability_cancels_it() {
    let mut app = app();
    reload(&mut app, |content| content.abilities.push(bolt()));
    let mage = actor(&mut app, []);
    app.world_mut().apply_effect(mage, Tome);
    app.tick();
    let ability = app.entity(mage).get::<Abilities>().iter().next().unwrap();
    app.world_mut().activate(ability).unwrap();
    app.tick();
    own(&mut app, mage, "state.stunned", true);
    app.tick();
    app.entity(ability).assert_has::<ActiveAbility>();

    reload(&mut app, |content| {
        content.abilities = vec![bolt().end_if(Condition::owner_has("state.stunned"))];
    });
    app.entity(ability).assert_lacks::<ActiveAbility>();
}

#[test]
fn a_reload_keeps_every_number_and_granted_abilities_working() {
    let mut app = app();
    let mage = caster(&mut app, [Fireball.spec()]);
    app.world_mut().apply_effect(mage, Staff);
    app.tick();
    let effects_before = app.resource::<EffectRegistry>().clone();
    let cost = effects_before.id("fireball.cost").expect("the cost");
    let staff = effects_before.id_of::<Staff>().expect("the staff");
    let fireball_id = app
        .resource::<AbilityRegistry>()
        .id_of::<Fireball>()
        .expect("fireball");

    // The same, and a new effect, named before every other.
    reload(&mut app, |content| {
        content
            .effects
            .push(EffectData::new("blast").flat::<Mana>(-1.0));
    });

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
fn content_defining_an_ability_code_defines_fails_the_check() {
    let app = app();
    let errors = MettleContent {
        abilities: vec![AbilityData::new("fireball").behaviour::<Cast>()],
        ..app.resource::<MettleContent>().clone()
    }
    .check(app.world())
    .unwrap_err();
    let defined = Error::DefinedInCode {
        kind: "ability",
        name: "fireball".to_owned(),
    };
    assert!(errors.0.contains(&defined), "{errors}");
    assert!(
        errors
            .to_string()
            .contains("content defines ability `fireball`, which code defines"),
        "{errors}"
    );
}

#[test]
fn an_ability_is_granted_at_its_specs_level() {
    let mut app = app();
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

    // By command.
    let third = actor(&mut app, []);
    app.world_mut()
        .commands()
        .entity(third)
        .grant_ability(Fireball.level(4.0));
    app.tick();
    let granted = app.entity(third).get::<Abilities>().iter().next().unwrap();
    assert!(close(level(&app, granted), 4.0));
}
