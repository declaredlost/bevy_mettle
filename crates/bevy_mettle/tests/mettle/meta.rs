//! Meta attributes: dealt by effects, routed into attributes actors have, in
//! one place; and what an execution tells it changed.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::attribute::registry::AnyAttribute;
use bevy_mettle::prelude::*;

use crate::common::{Damage, Life, Mana, Strength, actor, attributes, close, get, set_base};

/// A shield: what damage takes first.
#[derive(Attribute)]
#[attribute(min = 0.0)]
struct Shield;

/// Damage dealt, before the shield takes its share: what hits deal.
#[derive(MetaAttribute)]
struct IncomingDamage;

/// A price paid in life: what a blood spell costs.
#[derive(MetaAttribute)]
#[meta_attribute(name = "blood_price")]
struct BloodPrice;

/// The shield takes what it can, life the rest, and the source steals a
/// fifth of what life lost.
fn route_damage(context: &EffectContext, damage: f32) -> Vec<BaseChange> {
    let shield = context
        .value::<Shield>(context.target, &AttributeContext::new())
        .unwrap_or_default();
    let absorbed = damage.min(shield);
    let taken = damage - absorbed;
    vec![
        BaseChange::new::<Shield>(Operation::Flat, -absorbed),
        BaseChange::new::<Life>(Operation::Flat, -taken),
        BaseChange::new::<Life>(Operation::Flat, taken / 5.0).on_source(),
    ]
}

/// A price paid from life.
fn route_price(_: &EffectContext, price: f32) -> Vec<BaseChange> {
    vec![BaseChange::new::<Life>(Operation::Flat, -price)]
}

const TAGS: &[&str] = &["state"];

fn tags() -> TagTree {
    let defs: Vec<Tag> = TAGS
        .iter()
        .map(|&id| Tag {
            id: Id::from_static(id),
        })
        .collect();
    TagTree::build(&defs).unwrap()
}

/// The attributes the tests add, the shield with them.
fn with_shield(app: &mut App) {
    attributes(app);
    app.add_attribute::<Shield>();
}

fn code() -> EffectCode {
    EffectCode::new()
        .meta_attribute::<IncomingDamage>(route_damage)
        .meta_attribute::<BloodPrice>(route_price)
        .execution("strike", |context: &EffectContext| {
            let damage = context
                .source()
                .and_then(|source| context.value::<Damage>(source, &AttributeContext::new()))
                .unwrap_or_default();
            vec![BaseChange::meta::<IncomingDamage>(damage)]
        })
}

fn check(effects: &[Effect], code: &EffectCode) -> Result<EffectRegistry, MettleErrors> {
    let mut app = App::new();
    with_shield(&mut app);
    EffectRegistry::check(
        effects,
        EffectChecks {
            tags: &tags(),
            curves: &CurveTable::default(),
            attributes: app.world().resource::<AttributeRegistry>(),
            code,
        },
    )
}

/// An app with the shield, `effects` and the routers.
fn app(effects: &[Effect]) -> TestApp {
    let registry = check(effects, &code()).unwrap();
    TestApp::builder()
        .with(with_shield)
        .plugins(EffectPlugin::default())
        .with(|app| {
            app.insert_resource(registry);
        })
        .build()
}

/// An actor with every attribute, a shield of `shield`, and `life`.
fn shielded(app: &mut TestApp, shield: f32, life: f32) -> Entity {
    let actor = actor(app, []);
    app.entity_mut(actor).insert(Base::<Shield>::new(shield));
    set_base::<Life>(app, actor, life);
    app.tick();
    actor
}

fn hit(damage: f32) -> Effect {
    Effect::instant("hit").modifier(EffectModifier::meta::<IncomingDamage>(damage))
}

#[test]
fn damage_is_routed_through_the_shield_into_life() {
    let mut app = app(&[hit(40.0)]);
    app.watch_triggered::<EffectExecuted>();
    let npc = shielded(&mut app, 30.0, 50.0);
    let caster = shielded(&mut app, 0.0, 30.0);
    app.world_mut()
        .apply_effect(npc, EffectSpec::new("hit").source(caster));
    app.tick();
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 40.0), "10 past the shield");
    assert!(close(get::<Life>(&app, caster), 32.0), "a fifth stolen");

    let delta = |attribute: Id<AnyAttribute>, delta: f32| Delta { attribute, delta };
    app.assert_triggered::<EffectExecuted>(|executed| {
        executed.entity == npc
            && executed.source == Some(caster)
            && executed.changes == [delta(Shield::ID, -30.0), delta(Life::ID, -10.0)]
            && executed.source_changes == [delta(Life::ID, 2.0)]
            && executed.routed == [delta(IncomingDamage::ID, 40.0)]
    });
}

#[test]
fn an_execution_deals_a_meta_attribute_routed_the_same() {
    let mut app = app(&[Effect::instant("strike").execution("strike")]);
    let npc = shielded(&mut app, 5.0, 50.0);
    let caster = actor(&mut app, [Modifier::flat::<Damage>(15.0)]);
    app.world_mut()
        .apply_effect(npc, EffectSpec::new("strike").source(caster));
    app.tick();
    // 25 damage: 5 to the shield, 20 to life.
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 30.0));
}

#[test]
fn what_an_execution_changes_is_told_as_it_was_kept() {
    let mut app = app(&[hit(80.0)]);
    app.watch_triggered::<EffectExecuted>();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, EffectSpec::new("hit"));
    app.tick();
    assert!(close(get::<Life>(&app, npc), 0.0));
    // 80 dealt, but life had only 50 to lose; no source to steal it.
    app.assert_triggered::<EffectExecuted>(|executed| {
        executed
            .changes
            .iter()
            .any(|change| change.attribute == Life::ID && close(change.delta, -50.0))
            && executed.source_changes.is_empty()
            && executed.source.is_none()
    });
}

#[test]
fn routing_is_single_pass() {
    // A router dealing a meta attribute: what it deals of it is dropped.
    let code = code().meta_attribute::<IncomingDamage>(|_, damage| {
        vec![
            BaseChange::meta::<BloodPrice>(damage),
            BaseChange::new::<Mana>(Operation::Flat, -damage),
        ]
    });
    let registry = check(&[hit(10.0)], &code).unwrap();
    let mut app = TestApp::builder()
        .with(with_shield)
        .plugins(EffectPlugin::default())
        .with(|app| {
            app.insert_resource(registry);
        })
        .build();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, EffectSpec::new("hit"));
    app.tick();
    assert!(close(get::<Mana>(&app, npc), 20.0));
    assert!(close(get::<Life>(&app, npc), 50.0), "not routed again");
}

#[test]
fn a_preview_sees_what_meta_attributes_are_routed_into() {
    let price = Effect::instant("price").modifier(EffectModifier::meta::<BloodPrice>(20.0));
    let mut app = app(&[price]);
    let mage = shielded(&mut app, 0.0, 50.0);
    let changes = app
        .world()
        .preview_effect(mage, &EffectSpec::new("price"))
        .unwrap();
    assert_eq!(changes, [BaseChange::new::<Life>(Operation::Flat, -20.0)]);
}

#[test]
fn a_meta_attribute_is_dealt_flat_by_what_executes_and_never_read() {
    let lasting =
        Effect::lasting("aura", 5.0).modifier(EffectModifier::meta::<IncomingDamage>(1.0));
    let scaled = Effect::instant("scaled").modifier(EffectModifier::of_id(
        IncomingDamage::ID,
        Operation::Additive,
        0.5,
    ));
    let reads = Effect::instant("reads").modifier(EffectModifier::new::<Strength>(
        Operation::Flat,
        Magnitude::Attribute {
            attribute: IncomingDamage::ID,
            from: Capture::Source,
            coefficient: 1.0,
        },
    ));
    let periodic = Effect::lasting("bleed", 5.0)
        .period(1.0, false)
        .modifier(EffectModifier::meta::<IncomingDamage>(1.0));
    let errors = check(&[lasting, scaled, reads, periodic], &code())
        .unwrap_err()
        .to_string();
    for expected in [
        "meta attribute `incoming_damage` on a lasting effect with no period",
        "an Additive modifier of meta attribute `incoming_damage`",
        "reading meta attribute `incoming_damage`: it has no value",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
    assert!(
        !errors.contains("bleed"),
        "a periodic one executes: {errors}"
    );
}

#[test]
fn a_meta_attribute_cant_share_an_attributes_name() {
    #[derive(MetaAttribute)]
    #[meta_attribute(name = "life")]
    struct LifeLoss;

    let code = EffectCode::new().meta_attribute::<LifeLoss>(route_price);
    let errors = check(&[], &code).unwrap_err().to_string();
    assert!(
        errors.contains("an attribute and a meta attribute both named `life`"),
        "{errors}"
    );
}

/// The behaviour a blood bolt runs.
#[derive(Component, Default, Behaviour)]
struct Bolt;

#[test]
fn an_ability_paid_through_a_meta_attribute_is_checked_as_routed() {
    let price = Effect::instant("bolt.cost").modifier(EffectModifier::meta::<BloodPrice>(30.0));
    let effects = check(&[price], &code()).unwrap();
    let mut app = TestApp::builder()
        .with(with_shield)
        .plugins((EffectPlugin::default(), AbilityPlugin))
        .with(|app| {
            app.add_behaviour::<Bolt>();
        })
        .build();
    let abilities = AbilityRegistry::check(
        &[Ability::new("bolt", "bolt").cost("bolt.cost")],
        &[],
        AbilityChecks {
            tags: &tags(),
            effects: &effects,
            behaviours: app.world().resource::<Behaviours>(),
        },
    )
    .unwrap();
    app.insert_resource(effects);
    app.insert_resource(abilities);
    let mage = shielded(&mut app, 0.0, 20.0);
    let bolt = app.world_mut().grant_ability(mage, "bolt", 1.0).unwrap();
    app.tick();
    // 30 life to pay, 20 to pay it with.
    assert!(!app.world().can_afford(bolt));
    assert_eq!(
        app.world_mut().activate_ability(mage, "bolt"),
        Err(Failure::Cost)
    );
    set_base::<Life>(&mut app, mage, 50.0);
    app.tick();
    assert!(app.world_mut().activate_ability(mage, "bolt").is_ok());
    app.tick();
    assert!(close(get::<Life>(&app, mage), 20.0));
}
