//! Routes: amounts effects deal, routed into attributes actors have, in one
//! place; and what an execution tells it changed.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::Errors;
use bevy_mettle::core::registry::AttributeId;
use bevy_mettle::effect::events::Routed;
use bevy_mettle::prelude::*;

use crate::common::{Damage, Life, Mana, Strength, actor, attributes, close, get, id, set_base};

/// A shield: what damage takes first.
#[derive(Attribute)]
#[attribute(min = 0.0)]
struct Shield;

/// Damage dealt, before the shield takes its share: what hits deal, routed
/// by the router it's added with.
#[derive(Route)]
struct IncomingDamage(fn(&ExecutionContext, f32) -> Vec<BaseChange>);

impl RouteFn for IncomingDamage {
    fn route(&self, cx: &ExecutionContext, amount: f32) -> Vec<BaseChange> {
        (self.0)(cx, amount)
    }
}

/// A price paid in life: what a blood spell costs.
#[derive(Route)]
#[route(name = "blood_price")]
struct BloodPrice;

/// A price paid from life.
impl RouteFn for BloodPrice {
    fn route(&self, _: &ExecutionContext, price: f32) -> Vec<BaseChange> {
        vec![BaseChange::of::<Life>(Operation::Flat, -price)]
    }
}

/// A hit, dealing damage as content says.
#[derive(Effect)]
struct Hit;

/// A strike, an effect running [`StrikeDamage`].
#[derive(Effect)]
struct Strike;

/// The strike's execution, dealing the caster's damage.
#[derive(Execution)]
#[execution(name = "strike")]
struct StrikeDamage;

impl ExecutionFn for StrikeDamage {
    fn execute(&self, context: &ExecutionContext) -> Vec<BaseChange> {
        let damage = context
            .source()
            .and_then(|source| context.value::<Damage>(source, &AttributeContext::new()))
            .unwrap_or_default();
        vec![BaseChange::dealt::<IncomingDamage>(damage)]
    }
}

/// A price paid in life.
#[derive(Effect)]
struct Price;

/// An attribute no test adds.
#[derive(Attribute)]
struct Unadded;

/// A route no test adds.
#[derive(Route)]
struct Unrouted;

/// An effect running [`Stray`].
#[derive(Effect)]
struct Strayed;

/// An execution returning what it's added with: changes however named.
#[derive(Execution)]
#[execution(name = "stray")]
struct Stray(fn(&ExecutionContext) -> Vec<BaseChange>);

impl ExecutionFn for Stray {
    fn execute(&self, cx: &ExecutionContext) -> Vec<BaseChange> {
        (self.0)(cx)
    }
}

/// The blood bolt, as code names it.
#[derive(Ability)]
struct Bolt;

/// The shield takes what it can, life the rest, and the source steals a
/// fifth of what life lost.
fn route_damage(context: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
    let shield = context
        .value::<Shield>(context.target, &AttributeContext::new())
        .unwrap_or_default();
    let absorbed = damage.min(shield);
    let taken = damage - absorbed;
    vec![
        BaseChange::of::<Shield>(Operation::Flat, -absorbed),
        BaseChange::of::<Life>(Operation::Flat, -taken),
        BaseChange::of::<Life>(Operation::Flat, taken / 5.0).on_source(),
    ]
}

const TAGS: &[&str] = &["state"];

fn tags() -> TagRegistry {
    TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap()
}

/// The attributes the tests add, the shield with them.
fn with_shield(app: &mut App) {
    attributes(app);
    app.add_attribute::<Shield>();
}

/// The shield, the routers and the strike's execution, added to `app`.
fn code(app: &mut App) {
    with_shield(app);
    app.init_resource::<EffectRegistry>()
        .init_resource::<CalculationRegistry>()
        .add_route(IncomingDamage(route_damage))
        .add_route(BloodPrice)
        .add_execution(StrikeDamage);
}

/// `effects`, checked against what `world` has, into its effect registry.
fn check(world: &World, effects: &[EffectData]) -> Result<EffectRegistry, Errors> {
    world.resource::<EffectRegistry>().recheck(
        effects,
        EffectChecks {
            tags: &tags(),
            curves: &CurveRegistry::default(),
            attributes: world.resource(),
            executions: world.resource(),
            calculations: world.resource(),
            routes: world.resource(),
        },
    )
}

/// `effects` checked with nothing but the routes' code: what's wrong with them.
fn errors(effects: &[EffectData]) -> String {
    let mut app = App::new();
    code(&mut app);
    check(app.world(), effects).unwrap_err().to_string()
}

/// `app`'s effect registry, `effects` checked into it.
fn load(app: &mut App, effects: &[EffectData]) {
    let registry = check(app.world(), effects).unwrap();
    app.insert_resource(registry);
}

/// An app with the shield, `effects` and the routers, `E` the effect code
/// names.
fn app<E: Effect>(effects: &[EffectData]) -> TestApp {
    TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_effect::<E>();
            load(app, effects);
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

fn hit(damage: f32) -> EffectData {
    EffectData::instant(Key::of::<Hit>()).modifier(EffectModifier::route::<IncomingDamage>(damage))
}

#[test]
fn damage_is_routed_through_the_shield_into_life() {
    let mut app = app::<Hit>(&[hit(40.0)]);
    app.watch_triggered::<EffectExecuted>();
    let npc = shielded(&mut app, 30.0, 50.0);
    let caster = shielded(&mut app, 0.0, 30.0);
    app.world_mut().apply_effect(npc, Hit.source(caster));
    app.tick();
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 40.0), "10 past the shield");
    assert!(close(get::<Life>(&app, caster), 32.0), "a fifth stolen");

    let delta = |attribute: AttributeId, delta: f32| Delta { attribute, delta };
    let shield = app.world().attribute_id::<Shield>().unwrap();
    let route = app
        .world()
        .resource::<RouteRegistry>()
        .id_of::<IncomingDamage>()
        .unwrap();
    app.assert_triggered::<EffectExecuted>(|executed| {
        executed.entity == npc
            && executed.context.source == Some(caster)
            && executed.changes == [delta(shield, -30.0), delta(id::<Life>(), -10.0)]
            && executed.source_changes == [delta(id::<Life>(), 2.0)]
            && executed.routed
                == [Routed {
                    route,
                    amount: 40.0,
                }]
    });
}

#[test]
fn an_execution_deals_a_route_routed_the_same() {
    let mut app = app::<Strike>(&[
        EffectData::instant(Key::of::<Strike>()).execution(Key::of::<StrikeDamage>())
    ]);
    let npc = shielded(&mut app, 5.0, 50.0);
    let caster = actor(&mut app, [Modifier::flat(id::<Damage>(), 15.0)]);
    app.world_mut().apply_effect(npc, Strike.source(caster));
    app.tick();
    // 25 damage: 5 to the shield, 20 to life.
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 30.0));
}

#[test]
fn what_an_execution_changes_is_told_as_it_was_kept() {
    let mut app = app::<Hit>(&[hit(80.0)]);
    app.watch_triggered::<EffectExecuted>();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, Hit);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 0.0));
    // 80 dealt, but life had only 50 to lose; no source to steal it.
    app.assert_triggered::<EffectExecuted>(|executed| {
        executed
            .changes
            .iter()
            .any(|change| change.attribute == id::<Life>() && close(change.delta, -50.0))
            && executed.source_changes.is_empty()
            && executed.context.source.is_none()
    });
}

#[test]
fn routing_is_single_pass() {
    // A router dealing a route: what it deals it is dropped.
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_effect::<Hit>()
                .add_route(IncomingDamage(|_, damage| {
                    vec![
                        BaseChange::dealt::<BloodPrice>(damage),
                        BaseChange::of::<Mana>(Operation::Flat, -damage),
                    ]
                }));
            load(app, &[hit(10.0)]);
        })
        .build();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, Hit);
    app.tick();
    assert!(close(get::<Mana>(&app, npc), 20.0));
    assert!(close(get::<Life>(&app, npc), 50.0), "not routed again");
}

#[test]
fn a_preview_sees_what_routes_are_routed_into() {
    let price =
        EffectData::instant(Key::of::<Price>()).modifier(EffectModifier::route::<BloodPrice>(20.0));
    let mut app = app::<Price>(&[price]);
    let mage = shielded(&mut app, 0.0, 50.0);
    let changes = app.world().preview_effect(mage, &Price.spec()).unwrap();
    assert_eq!(
        changes,
        [BaseChange::new(id::<Life>(), Operation::Flat, -20.0)]
    );
}

#[test]
fn a_route_is_dealt_flat_by_what_executes_and_never_read() {
    let lasting =
        EffectData::lasting("aura", 5.0).modifier(EffectModifier::route::<IncomingDamage>(1.0));
    let mut additive = EffectModifier::route::<IncomingDamage>(0.5);
    additive.op = Operation::Additive;
    let scaled = EffectData::instant("scaled").modifier(additive);
    let reads = EffectData::instant("reads").modifier(EffectModifier::new::<Strength>(
        Operation::Flat,
        Magnitude::Attribute(AttributeRead {
            attribute: Key::from_static("incoming_damage"),
            from: Capture::Source,
            value: Read::Current,
            snapshot: true,
            tags: Tags::new(),
        }),
    ));
    let periodic = EffectData::lasting("bleed", 5.0)
        .period(1.0, false)
        .modifier(EffectModifier::route::<IncomingDamage>(1.0));
    let errors = errors(&[lasting, scaled, reads, periodic]);
    for expected in [
        "route `incoming_damage` on a lasting effect with no period",
        "an Additive modifier of route `incoming_damage`",
        "there's no attribute called `incoming_damage` (in `reads`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
    assert!(
        !errors.contains("bleed"),
        "a periodic one executes: {errors}"
    );
}

#[test]
fn a_modifier_changes_an_attribute_or_a_route() {
    let mut both = EffectModifier::route::<IncomingDamage>(1.0);
    both.attribute = Some(Key::of::<Life>());
    let mut neither = EffectModifier::route::<IncomingDamage>(1.0);
    neither.route = None;
    let errors = errors(&[
        EffectData::instant("both").modifier(both),
        EffectData::instant("neither").modifier(neither),
    ]);
    for expected in [
        "a modifier of both an attribute and a route: it's one or the other (in `both`)",
        "a modifier of neither an attribute nor a route (in `neither`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}

/// The behaviour a blood bolt runs.
#[derive(Component, Default, Behaviour)]
struct CastBolt;

#[test]
fn an_ability_paid_through_a_route_is_checked_as_routed() {
    let price =
        EffectData::instant("bolt.cost").modifier(EffectModifier::route::<BloodPrice>(30.0));
    let mut app = TestApp::builder()
        .plugins((EffectPlugin, AbilityPlugin))
        .with(|app| {
            code(app);
            app.add_behaviour::<CastBolt>().add_ability::<Bolt>();
            load(app, &[price]);
        })
        .build();
    let world = app.world();
    let abilities = world
        .resource::<AbilityRegistry>()
        .recheck(
            &[AbilityData::new(Key::of::<Bolt>(), Key::of::<CastBolt>()).cost("bolt.cost")],
            &[],
            AbilityChecks {
                tags: &tags(),
                effects: world.resource(),
                behaviours: world.resource(),
            },
        )
        .unwrap();
    app.insert_resource(abilities);
    let mage = shielded(&mut app, 0.0, 20.0);
    let bolt = app.world_mut().grant_ability(mage, Bolt).unwrap();
    app.tick();
    // 30 life to pay, 20 to pay it with.
    assert!(!app.world().can_afford(bolt));
    assert_eq!(
        app.world_mut().activate_ability(mage, Bolt),
        Err(Failure::Cost)
    );
    set_base::<Life>(&mut app, mage, 50.0);
    app.tick();
    assert!(app.world_mut().activate_ability(mage, Bolt).is_ok());
    app.tick();
    assert!(close(get::<Life>(&app, mage), 20.0));
}

/// An actor with a shield of 5 and 50 life, struck by a [`Stray`] running
/// `run`.
fn strayed(run: fn(&ExecutionContext) -> Vec<BaseChange>) -> (TestApp, Entity) {
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(move |app| {
            code(app);
            app.add_execution(Stray(run)).add_effect::<Strayed>();
            load(
                app,
                &[EffectData::instant(Key::of::<Strayed>()).execution(Key::of::<Stray>())],
            );
        })
        .build();
    let npc = shielded(&mut app, 5.0, 50.0);
    app.world_mut().apply_effect(npc, Strayed);
    app.tick();
    (app, npc)
}

#[test]
fn a_change_to_an_attribute_never_added_is_dropped() {
    let (app, npc) = strayed(|_| vec![BaseChange::of::<Unadded>(Operation::Flat, -10.0)]);
    assert!(close(get::<Shield>(&app, npc), 5.0));
    assert!(close(get::<Life>(&app, npc), 50.0));
}

#[test]
fn what_is_dealt_a_route_never_added_is_dropped() {
    let (app, npc) = strayed(|_| vec![BaseChange::dealt::<Unrouted>(40.0)]);
    assert!(close(get::<Shield>(&app, npc), 5.0));
    assert!(close(get::<Life>(&app, npc), 50.0));
}

#[test]
fn a_change_by_type_or_by_number_is_the_same() {
    let typed = strayed(|_| {
        vec![
            BaseChange::of::<Life>(Operation::Flat, -10.0),
            BaseChange::dealt::<IncomingDamage>(8.0),
        ]
    });
    let numbered = strayed(|cx| {
        vec![
            BaseChange::new(cx.attribute_id::<Life>().unwrap(), Operation::Flat, -10.0),
            BaseChange::route(cx.route_id::<IncomingDamage>().unwrap(), 8.0),
        ]
    });
    for (app, npc) in [typed, numbered] {
        // 8 dealt: 5 to the shield, 3 to life, with the 10 taken outright.
        assert!(close(get::<Shield>(&app, npc), 0.0));
        assert!(close(get::<Life>(&app, npc), 37.0));
    }
}
