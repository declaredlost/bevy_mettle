//! Routes: amounts effects route into, routed into attributes actors have, in
//! one place; and what an execution tells it changed.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::Errors;
use bevy_mettle::core::registry::AttributeId;
use bevy_mettle::effect::EffectScope;
use bevy_mettle::effect::code::{CalculationRegistry, Changes, EffectGroup, RouteRegistry};
use bevy_mettle::effect::context::ContextDataRegistry;
use bevy_mettle::effect::events::Routed;
use bevy_mettle::prelude::*;

use crate::common::{Damage, Life, Mana, Strength, actor, attributes, close, get, id, set_base};

/// A shield: what damage takes first.
#[derive(Attribute)]
#[attribute(min = 0.0)]
struct Shield;

/// Damage dealt, before the shield takes its share: what hits route into it,
/// routed by the router it's added with, the target's shield read as it's
/// routed.
#[derive(Route)]
#[route(target(Shield, live))]
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

/// A hit, dealing 40 damage.
#[derive(Effect)]
#[effect(route::<IncomingDamage>(40.0))]
struct Hit;

/// A crushing hit, dealing 80 damage.
#[derive(Effect)]
#[effect(route::<IncomingDamage>(80.0))]
struct Crush;

/// A grazing hit, dealing 10 damage.
#[derive(Effect)]
#[effect(route::<IncomingDamage>(10.0))]
struct Graze;

/// A strike, an effect running [`StrikeDamage`].
#[derive(Effect)]
#[effect(execution::<StrikeDamage>)]
struct Strike;

/// The strike's execution, routing the caster's damage into incoming damage.
#[derive(Execution)]
#[execution(name = "strike", source(Damage), routes(IncomingDamage))]
struct StrikeDamage;

impl ExecutionFn for StrikeDamage {
    fn execute(&self, context: &ExecutionContext) -> Vec<BaseChange> {
        let damage = context.captured::<Damage>(Side::Source).unwrap_or_default();
        vec![BaseChange::route::<IncomingDamage>(damage)]
    }
}

/// A price paid in life.
#[derive(Effect)]
#[effect(route::<BloodPrice>(20.0))]
struct Price;

/// Damage made more by the source's strength as the effect was made.
#[derive(Route)]
#[route(source(Strength))]
struct Empowered;

impl RouteFn for Empowered {
    fn route(&self, cx: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
        let strength = cx.captured::<Strength>(Side::Source).unwrap_or_default();
        vec![BaseChange::of::<Life>(
            Operation::Flat,
            -(damage + strength),
        )]
    }
}

/// A smite, routing into [`Empowered`].
#[derive(Effect)]
#[effect(route::<Empowered>(2.0))]
struct Smite;

/// A burn, routing into incoming damage as it's applied and each second.
#[derive(Effect)]
#[effect(lasting = 10.0, period = 1.0, execute_on_apply)]
#[effect(route::<IncomingDamage>(10.0))]
struct Searing;

/// An attribute no test adds.
#[derive(Attribute)]
struct Unadded;

/// A route no test adds.
#[derive(Route)]
struct Unrouted;

/// An effect running [`Stray`].
#[derive(Effect)]
#[effect(execution::<Stray>)]
struct Strayed;

/// An execution returning what it's added with: changes however named, into
/// incoming damage as it declares.
#[derive(Execution)]
#[execution(name = "stray", routes(IncomingDamage))]
struct Stray(fn(&ExecutionContext) -> Vec<BaseChange>);

/// A route whose router captures what no test adds.
#[derive(Route)]
#[route(source(Unadded))]
struct Broken;

impl RouteFn for Broken {
    fn route(&self, _: &ExecutionContext, _: f32) -> Vec<BaseChange> {
        Vec::new()
    }
}

/// An execution routing into [`Broken`].
#[derive(Execution)]
#[execution(routes(Broken))]
struct Breaking;

impl ExecutionFn for Breaking {
    fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
        vec![BaseChange::route::<Broken>(1.0)]
    }
}

/// An execution routing into a route no test adds.
#[derive(Execution)]
#[execution(routes(Unrouted))]
struct Lost;

impl ExecutionFn for Lost {
    fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
        vec![BaseChange::route::<Unrouted>(1.0)]
    }
}

impl ExecutionFn for Stray {
    fn execute(&self, cx: &ExecutionContext) -> Vec<BaseChange> {
        (self.0)(cx)
    }
}

/// The shield takes what it can, life the rest, and the source steals a
/// fifth of what life lost.
fn route_damage(context: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
    let shield = context.captured::<Shield>(Side::Target).unwrap_or_default();
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
    world.resource::<EffectRegistry>().rebuild(
        effects,
        EffectScope {
            tags: &tags(),
            curves: &CurveRegistry::default(),
            attributes: world.resource(),
            executions: world.resource(),
            calculations: world.resource(),
            routes: world.resource(),
            context: &ContextDataRegistry::default(),
        },
    )
}

/// `effects` checked with nothing but the routes' code: what's wrong with them.
fn errors(effects: &[EffectData]) -> String {
    let mut app = App::new();
    code(&mut app);
    check(app.world(), effects).unwrap_err().to_string()
}

/// `app`'s effect registry, the effects code defines compiled into it.
fn load(app: &mut App) {
    let registry = check(app.world(), &[]).unwrap();
    app.insert_resource(registry);
}

/// An app with the shield, the routers and `E`, the effects code defines.
fn app<E: EffectGroup>() -> TestApp {
    TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_effects::<E>();
            load(app);
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

#[test]
fn damage_is_routed_through_the_shield_into_life() {
    let mut app = app::<Hit>();
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
fn an_execution_routes_into_a_route_routed_the_same() {
    let mut app = app::<Strike>();
    let npc = shielded(&mut app, 5.0, 50.0);
    let caster = actor(&mut app, [ModifierSpec::flat::<Damage>(15.0)]);
    app.world_mut().apply_effect(npc, Strike.source(caster));
    app.tick();
    // 25 damage: 5 to the shield, 20 to life.
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 30.0));
}

#[test]
fn what_an_execution_changes_is_told_as_it_was_kept() {
    let mut app = app::<Crush>();
    app.watch_triggered::<EffectExecuted>();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, Crush);
    app.tick();
    assert!(close(get::<Life>(&app, npc), 0.0));
    // 80 routed, but life had only 50 to lose; no source to steal it.
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
    // A router routing into a route: what it routes into it is dropped.
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_effect::<Graze>()
                .add_route(IncomingDamage(|_, damage| {
                    vec![
                        BaseChange::route::<BloodPrice>(damage),
                        BaseChange::of::<Mana>(Operation::Flat, -damage),
                    ]
                }));
            load(app);
        })
        .build();
    let npc = shielded(&mut app, 0.0, 50.0);
    app.world_mut().apply_effect(npc, Graze);
    app.tick();
    assert!(close(get::<Mana>(&app, npc), 20.0));
    assert!(close(get::<Life>(&app, npc), 50.0), "not routed again");
}

#[test]
fn a_preview_sees_what_routes_are_routed_into() {
    let mut app = app::<Price>();
    let mage = shielded(&mut app, 0.0, 50.0);
    let changes = app.world().preview_effect(mage, &Price.spec()).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].changes(),
        Some(Changes::Attribute(id::<Life>())),
        "numbered"
    );
    assert_eq!((changes[0].op, changes[0].value), (Operation::Flat, -20.0));
}

#[test]
fn a_route_is_routed_into_flat_by_what_executes_and_never_read() {
    let lasting = EffectData::new("aura")
        .lasting(5.0)
        .route::<IncomingDamage>(1.0);
    let mut additive = EffectModifier::route::<IncomingDamage>(0.5);
    additive.op = Operation::Additive;
    let scaled = EffectData::new("scaled").modifier(additive);
    let reads = EffectData::new("reads").modifier(EffectModifier::new::<Strength>(
        Operation::Flat,
        Magnitude::Attribute(AttributeRead {
            attribute: Key::from_static("incoming_damage"),
            from: Side::Source,
            value: Read::Current,
            snapshot: true,
            tags: Tags::new(),
        }),
    ));
    let periodic = EffectData::new("bleed")
        .lasting(5.0)
        .period(1.0)
        .route::<IncomingDamage>(1.0);
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
        EffectData::new("both").modifier(both),
        EffectData::new("neither").modifier(neither),
    ]);
    for expected in [
        "a modifier of both an attribute and a route: it's one or the other (in `both`)",
        "a modifier of neither an attribute nor a route (in `neither`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}

/// An actor with a shield of 5 and 50 life, struck by a [`Stray`] running
/// `run`.
fn strayed(run: fn(&ExecutionContext) -> Vec<BaseChange>) -> (TestApp, Entity) {
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(move |app| {
            code(app);
            app.add_execution(Stray(run)).add_effect::<Strayed>();
            load(app);
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
fn what_is_routed_into_a_route_never_added_is_dropped() {
    let (app, npc) = strayed(|_| vec![BaseChange::route::<Unrouted>(40.0)]);
    assert!(close(get::<Shield>(&app, npc), 5.0));
    assert!(close(get::<Life>(&app, npc), 50.0));
}

#[test]
fn what_an_execution_routes_into_undeclared_is_dropped() {
    // Blood price is added, but `Stray` declares only incoming damage.
    let (app, npc) = strayed(|_| {
        vec![
            BaseChange::route::<BloodPrice>(20.0),
            BaseChange::of::<Mana>(Operation::Flat, -5.0),
        ]
    });
    assert!(close(get::<Shield>(&app, npc), 5.0));
    assert!(close(get::<Life>(&app, npc), 50.0), "the price dropped");
    assert!(close(get::<Mana>(&app, npc), 25.0), "the rest kept");
}

#[test]
fn an_effect_captures_what_the_routes_it_routes_into_capture() {
    let mut app = App::new();
    code(&mut app);
    app.add_route(Empowered).add_effect::<Strike>();
    let registry = check(app.world(), &[]).unwrap();
    let def = registry.get(registry.id_of::<Strike>().unwrap()).unwrap();
    let routes = app.world().resource::<RouteRegistry>();
    let incoming = routes.id_of::<IncomingDamage>().unwrap();
    assert_eq!(
        def.routes
            .iter()
            .map(|(route, _)| *route)
            .collect::<Vec<_>>(),
        [incoming],
        "only what it declares"
    );
    let shield = app.world().attribute_id::<Shield>().unwrap();
    assert_eq!(
        def.captures
            .iter()
            .map(|capture| (capture.attribute, capture.from))
            .collect::<Vec<_>>(),
        [(id::<Damage>(), Side::Source), (shield, Side::Target)],
        "its own and incoming damage's, not empowered's strength"
    );
}

#[test]
fn a_route_capturing_what_isnt_added_rejects_only_what_routes_into_it() {
    let mut app = App::new();
    code(&mut app);
    app.add_route(Broken).add_execution(Breaking);
    let strike = EffectData::new("strike").execution::<StrikeDamage>();
    let by_modifier = EffectData::new("by_modifier").route::<Broken>(1.0);
    let by_execution = EffectData::new("by_execution").execution::<Breaking>();
    let errors = check(app.world(), &[strike.clone(), by_modifier, by_execution])
        .unwrap_err()
        .to_string();
    for expected in [
        "there's no attribute called `unadded` (in `by_modifier`)",
        "there's no attribute called `unadded` (in `by_execution`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
    assert!(!errors.contains("(in `strike`)"), "{errors}");
    assert!(check(app.world(), &[strike]).is_ok());
}

#[test]
fn an_execution_routing_into_a_route_never_added_is_rejected() {
    let mut app = App::new();
    code(&mut app);
    app.add_execution(Lost);
    let lost = EffectData::new("lost").execution::<Lost>();
    let errors = check(app.world(), &[lost]).unwrap_err().to_string();
    assert!(
        errors.contains(
            "execution `lost` routes into `unrouted`, which isn't an added route (in `lost`)"
        ),
        "{errors}"
    );
}

#[test]
fn a_change_by_type_is_numbered_as_it_applies() {
    let (app, npc) = strayed(|_| {
        vec![
            BaseChange::of::<Life>(Operation::Flat, -10.0),
            BaseChange::route::<IncomingDamage>(8.0),
        ]
    });
    // 8 routed: 5 to the shield, 3 to life, with the 10 taken outright.
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(close(get::<Life>(&app, npc), 37.0));
}

#[test]
fn a_router_reads_what_it_captures_as_the_effect_was_made() {
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_route(Empowered).add_effect::<Smite>();
            load(app);
        })
        .build();
    let npc = shielded(&mut app, 0.0, 50.0);
    let caster = shielded(&mut app, 0.0, 50.0);
    set_base::<Strength>(&mut app, caster, 5.0);
    app.tick();
    // Snapshotted as it's made, then the caster grows stronger.
    let made = app.world().snapshot(Smite.source(caster));
    set_base::<Strength>(&mut app, caster, 10.0);
    app.tick();
    app.world_mut().apply_effect(npc, made);
    app.tick();
    assert!(
        close(get::<Life>(&app, npc), 43.0),
        "2, and the 5 it was made with"
    );
    app.world_mut().apply_effect(npc, Smite.source(caster));
    app.tick();
    assert!(close(get::<Life>(&app, npc), 31.0), "2, and the 10 it has");
}

#[test]
fn a_router_follows_what_it_captures_live_each_period() {
    let mut app = TestApp::builder()
        .plugins(EffectPlugin)
        .with(|app| {
            code(app);
            app.add_effect::<Searing>();
            load(app);
        })
        .build();
    let npc = shielded(&mut app, 15.0, 50.0);
    let caster = shielded(&mut app, 0.0, 30.0);
    app.world_mut().apply_effect(npc, Searing.source(caster));
    app.tick();
    assert!(close(get::<Shield>(&app, npc), 5.0));
    assert!(
        close(get::<Life>(&app, npc), 50.0),
        "the shield took it all"
    );
    app.secs(1.0);
    assert!(close(get::<Shield>(&app, npc), 0.0));
    assert!(
        close(get::<Life>(&app, npc), 45.0),
        "the shield as it is now took 5: {}",
        get::<Life>(&app, npc)
    );
}
