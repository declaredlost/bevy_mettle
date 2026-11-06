//! Effects: applied, lasting, periodic, stacked, inhibited, removed,
//! chained, checked and reloaded.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::prelude::*;

use crate::common::{
    AttackSpeed, Damage, FIRE, Life, Resistance, Strength, actor, attributes, close, get, own,
};

/// The tags the effects name.
const TAGS: &[&str] = &[
    "damage",
    "effect",
    "effect.affliction",
    "effect.affliction.burning",
    "effect.affliction.poison",
    "effect.buff",
    "state",
    "state.burning",
    "state.wet",
    "state.enraged",
    "immune",
    "immune.fire",
    "param",
    "param.damage",
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

fn curves() -> CurveTable {
    CurveTable::build(&[LevelCurve {
        id: Id::from_static("damage"),
        interpolation: Interpolation::Linear,
        keys: vec![(1.0, 10.0), (11.0, 110.0)],
    }])
    .unwrap()
}

/// The attributes the tests add.
fn attribute_registry() -> AttributeRegistry {
    let mut app = App::new();
    attributes(&mut app);
    app.world().resource::<AttributeRegistry>().clone()
}

fn registry(effects: &[Effect], code: &EffectCode) -> EffectRegistry {
    EffectRegistry::build(effects, &tags(), &curves(), &attribute_registry(), code).unwrap()
}

/// An app with attributes and effects, `effects` in its registry.
fn app(effects: &[Effect]) -> TestApp {
    app_with(effects, &EffectCode::new())
}

fn app_with(effects: &[Effect], code: &EffectCode) -> TestApp {
    let registry = registry(effects, code);
    TestApp::builder()
        .with(attributes)
        .plugins(EffectPlugin::default())
        .with(|app| {
            app.insert_resource(registry);
        })
        .build()
}

fn apply(app: &mut TestApp, target: Entity, spec: EffectSpec) {
    app.world_mut().apply_effect(target, spec);
    app.tick();
}

fn flat<A: Attribute>(value: f32) -> EffectModifier {
    EffectModifier::new::<A>(Operation::Flat, value)
}

fn effects_on(app: &TestApp, target: Entity) -> Vec<Entity> {
    app.world()
        .get::<ActiveEffects>(target)
        .map(|effects| effects.iter().collect())
        .unwrap_or_default()
}

fn owns(app: &TestApp, target: Entity, tag: &'static str) -> bool {
    app.world()
        .get::<OwnedTags>(target)
        .unwrap()
        .contains(&Id::from_static(tag))
}

/// What was told, in order.
#[derive(Resource, Default)]
struct Told(Vec<String>);

fn watch(app: &mut TestApp) {
    let world = app.world_mut();
    world.init_resource::<Told>();
    world.add_observer(|event: On<EffectApplied>, mut told: ResMut<Told>| {
        told.0.push(format!("applied {}", event.id));
    });
    world.add_observer(|event: On<EffectBlocked>, mut told: ResMut<Told>| {
        told.0
            .push(format!("blocked {} {:?}", event.id, event.reason));
    });
    world.add_observer(|event: On<EffectStacked>, mut told: ResMut<Told>| {
        told.0
            .push(format!("stacked {} {}", event.id, event.stacks));
    });
    world.add_observer(|event: On<EffectRemoved>, mut told: ResMut<Told>| {
        told.0
            .push(format!("removed {} {}", event.id, event.expired));
    });
}

fn told(app: &mut TestApp) -> Vec<String> {
    std::mem::take(&mut app.world_mut().resource_mut::<Told>().0)
}

#[test]
fn an_instant_effect_changes_the_base_within_its_limits() {
    let hit = Effect::instant("hit").modifier(flat::<Life>(-20.0));
    let crush = Effect::instant("crush").modifier(flat::<Life>(-100.0));
    let mut app = app(&[hit, crush]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("hit"));
    assert!(close(get::<Life>(&app, npc), 30.0));
    apply(&mut app, npc, EffectSpec::new("crush"));
    let base = app.world().get::<Base<Life>>(npc).unwrap().get();
    assert!(
        close(base, 0.0),
        "the base itself kept at least none: {base}"
    );
    assert!(
        effects_on(&app, npc).is_empty(),
        "an instant one doesn't last"
    );
}

#[test]
fn a_lasting_effect_counts_and_grants_while_it_lasts() {
    let haste = Effect::lasting("haste", 1.0)
        .modifier(EffectModifier::new::<AttackSpeed>(Operation::Additive, 0.5))
        .grant("effect.buff");
    let mut app = app(&[haste]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("haste"));
    assert!(close(get::<AttackSpeed>(&app, npc), 1.5));
    assert!(owns(&app, npc, "effect.buff"));
    app.secs(1.0);
    assert!(close(get::<AttackSpeed>(&app, npc), 1.0), "run out");
    assert!(!owns(&app, npc, "effect.buff"));
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn a_periodic_effect_executes_each_period_and_on_apply() {
    let burning = Effect::lasting("burning", 2.0)
        .period(0.5, true)
        .modifier(flat::<Life>(-1.0));
    let mut app = app(&[burning]);
    let npc = actor(&mut app, []);
    app.world_mut()
        .apply_effect(npc, EffectSpec::new("burning"));
    assert!(close(get::<Life>(&app, npc), 49.0), "on apply");
    app.secs(2.0);
    // On apply, then at 0.5, 1, 1.5 and 2.
    assert!(
        close(get::<Life>(&app, npc), 45.0),
        "{}",
        get::<Life>(&app, npc)
    );
}

#[test]
fn a_requirement_or_an_immunity_blocks_an_effect() {
    let burning = Effect::lasting("burning", 4.0)
        .tag("effect.affliction.burning")
        .require(!Condition::owner_has("state.wet"));
    let warded = Effect::infinite("warded").immunity("effect.affliction");
    let mut app = app(&[burning, warded]);
    watch(&mut app);
    let npc = actor(&mut app, []);
    own(&mut app, npc, "state.wet", true);
    apply(&mut app, npc, EffectSpec::new("burning"));
    own(&mut app, npc, "state.wet", false);
    apply(&mut app, npc, EffectSpec::new("warded"));
    apply(&mut app, npc, EffectSpec::new("burning"));
    assert_eq!(
        told(&mut app),
        [
            "blocked burning Requirement",
            "applied warded",
            "blocked burning Immunity"
        ]
    );
}

#[test]
fn an_effect_removes_those_tagged_as_it_says() {
    let poison = Effect::lasting("poison", 10.0).tag("effect.affliction.poison");
    let cure = Effect::instant("cure").removes("effect.affliction");
    let mut app = app(&[poison, cure]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("poison"));
    assert_eq!(effects_on(&app, npc).len(), 1);
    apply(&mut app, npc, EffectSpec::new("cure"));
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn stacks_count_their_modifiers_up_to_a_limit_then_overflow() {
    let rage = Effect::lasting("rage", 1.0)
        .modifier(flat::<Damage>(2.0))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 3,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::RemoveOne,
            overflow: vec![Id::from_static("frenzy")],
        });
    let frenzy = Effect::instant("frenzy").modifier(flat::<Strength>(1.0));
    let mut app = app(&[rage, frenzy]);
    watch(&mut app);
    let npc = actor(&mut app, []);
    for _ in 0..4 {
        apply(&mut app, npc, EffectSpec::new("rage"));
    }
    assert!(close(get::<Damage>(&app, npc), 16.0), "three stacks");
    assert!(close(get::<Strength>(&app, npc), 11.0), "overflowed once");
    assert_eq!(effects_on(&app, npc).len(), 1);
    assert_eq!(
        told(&mut app),
        [
            "applied rage",
            "stacked rage 2",
            "stacked rage 3",
            "blocked rage StackLimit"
        ]
    );
    app.secs(1.0);
    assert!(close(get::<Damage>(&app, npc), 14.0), "one stack off");
}

#[test]
fn an_effect_is_inhibited_while_its_ongoing_requirement_fails() {
    let rage = Effect::infinite("rage")
        .modifier(flat::<Damage>(5.0))
        .grant("state.enraged")
        .ongoing(!Condition::owner_has("state.wet"));
    let mut app = app(&[rage]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("rage"));
    assert!(close(get::<Damage>(&app, npc), 15.0));
    own(&mut app, npc, "state.wet", true);
    app.ticks(2);
    assert!(close(get::<Damage>(&app, npc), 10.0), "inhibited");
    assert!(!owns(&app, npc, "state.enraged"));
    own(&mut app, npc, "state.wet", false);
    app.ticks(2);
    assert!(close(get::<Damage>(&app, npc), 15.0), "counting again");
    assert!(owns(&app, npc, "state.enraged"));
}

#[test]
fn an_effect_comes_off_once_its_removal_requirement_holds() {
    let burning = Effect::infinite("burning").remove_if(Condition::owner_has("state.wet"));
    let mut app = app(&[burning]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("burning"));
    own(&mut app, npc, "state.wet", true);
    app.ticks(2);
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn magnitudes_come_from_curves_params_attributes_and_code() {
    let curve = Effect::instant("curve").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::scaled(0.5, "damage"),
    ));
    let param = Effect::instant("param").modifier(EffectModifier::new::<Life>(
        Operation::Flat,
        Magnitude::param("param.damage"),
    ));
    let captured = Effect::instant("captured").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::source::<Strength>(0.5),
    ));
    let custom = Effect::instant("custom").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::Custom(Id::from_static("level_squared")),
    ));
    let code = EffectCode::new().calculation("level_squared", |context: &EffectContext| {
        context.level() * context.level()
    });
    let mut app = app_with(&[curve, param, captured, custom], &code);
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    // 0.5 × the curve at 6: 60.
    apply(&mut app, npc, EffectSpec::new("curve").level(6.0));
    assert!(close(get::<Damage>(&app, npc), 40.0));
    apply(
        &mut app,
        npc,
        EffectSpec::new("param").param("param.damage", -12.0),
    );
    assert!(close(get::<Life>(&app, npc), 38.0));
    // Half the caster's strength of 10.
    apply(&mut app, npc, EffectSpec::new("captured").source(caster));
    assert!(close(get::<Damage>(&app, npc), 45.0));
    apply(&mut app, npc, EffectSpec::new("custom").level(3.0));
    assert!(close(get::<Damage>(&app, npc), 54.0));
}

#[test]
fn an_execution_works_damage_out_from_both_sides() {
    let strike = Effect::instant("strike").execution("strike");
    let code = EffectCode::new().execution("strike", |context: &EffectContext| {
        let query = AttributeContext::new();
        let damage = context
            .source()
            .and_then(|source| context.value::<Damage>(source, &query))
            .unwrap_or_default();
        let resist = context
            .value::<Resistance>(context.target, &AttributeContext::new().tag(FIRE))
            .unwrap_or_default();
        vec![BaseChange::new::<Life>(
            Operation::Flat,
            -damage * (1.0 - resist),
        )]
    });
    let mut app = app_with(&[strike], &code);
    let npc = actor(
        &mut app,
        [Modifier::flat::<Resistance>(0.5).read_tagged(FIRE)],
    );
    let caster = actor(&mut app, [Modifier::flat::<Damage>(10.0)]);
    apply(&mut app, npc, EffectSpec::new("strike").source(caster));
    // 20 damage, half resisted.
    assert!(close(get::<Life>(&app, npc), 40.0));
}

#[test]
fn effects_apply_effects_as_they_apply_and_expire() {
    let fuse = Effect::lasting("fuse", 0.5)
        .on_apply("spark")
        .on_expire("blast");
    let spark = Effect::instant("spark").modifier(flat::<Life>(-1.0));
    let blast = Effect::instant("blast").modifier(flat::<Life>(-10.0));
    let mut app = app(&[fuse, spark, blast]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("fuse"));
    assert!(close(get::<Life>(&app, npc), 49.0));
    app.secs(0.5);
    assert!(close(get::<Life>(&app, npc), 39.0));
}

#[test]
fn a_source_term_counts_as_the_source_was_when_applied() {
    let rally = Effect::infinite("rally")
        .modifier(flat::<Damage>(5.0).condition(Condition::source_has("state.enraged")));
    let mut app = app(&[rally]);
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    own(&mut app, caster, "state.enraged", true);
    apply(&mut app, npc, EffectSpec::new("rally").source(caster));
    own(&mut app, caster, "state.enraged", false);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 15.0), "as it was");
    apply(&mut app, npc, EffectSpec::new("rally").source(caster));
    assert!(
        close(get::<Damage>(&app, npc), 15.0),
        "a calm one adds nothing"
    );
}

#[test]
fn effects_come_off_by_activation_tag_or_with_their_actor() {
    let ward = Effect::infinite("ward").tag("effect.buff");
    let mut app = app(&[ward]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("ward").activation(7));
    apply(&mut app, npc, EffectSpec::new("ward"));
    app.world_mut().commands().entity(npc).remove_activation(7);
    app.tick();
    assert_eq!(
        effects_on(&app, npc).len(),
        1,
        "the predicted one taken back"
    );
    let last = effects_on(&app, npc)[0];
    app.world_mut().despawn(npc);
    assert!(app.world().get_entity(last).is_err(), "gone with its actor");
}

#[test]
fn a_new_registry_gives_active_effects_their_new_definitions() {
    let haste = Effect::infinite("haste").modifier(flat::<AttackSpeed>(1.0));
    let mut app = app(&[haste]);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, EffectSpec::new("haste"));
    assert!(close(get::<AttackSpeed>(&app, npc), 2.0));
    let faster = Effect::infinite("haste").modifier(flat::<AttackSpeed>(3.0));
    app.world_mut()
        .insert_resource(registry(&[faster], &EffectCode::new()));
    app.ticks(2);
    assert!(close(get::<AttackSpeed>(&app, npc), 4.0));
    app.world_mut()
        .insert_resource(registry(&[], &EffectCode::new()));
    app.ticks(2);
    assert!(effects_on(&app, npc).is_empty(), "its effect is gone");
}

#[test]
fn an_effect_reads_from_data_as_written() {
    let def: Effect = ron::from_str(
        r#"(
            id: "burning",
            tags: ["effect.affliction.burning"],
            duration: For(Value(4)),
            period: (every: Value(0.5), on_apply: true),
            modifiers: [(attribute: "life", op: Flat, value: Param("param.damage"))],
            grants: ["state.burning"],
            require: Not(OwnerHas("immune.fire")),
            ongoing: Not(OwnerHas("state.wet")),
            stacking: (limit: 3, expire: RemoveOne),
        )"#,
    )
    .unwrap();
    assert!(
        registry(&[def], &EffectCode::new())
            .get(&Id::from_static("burning"))
            .is_some()
    );
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    let focus: EffectModifier =
        ron::from_str(r#"(attribute: "focus", op: Flat, value: Value(1))"#).unwrap();
    let bad = Effect::instant("bad")
        .modifier(focus)
        .tag("effect.unknown")
        .grant("state.burning")
        .execution("nowhere")
        .on_apply("missing")
        .modifier(EffectModifier::new::<Damage>(
            Operation::Flat,
            Magnitude::scaled(1.0, "nope"),
        ));
    let errors = EffectRegistry::build(
        &[bad],
        &tags(),
        &curves(),
        &attribute_registry(),
        &EffectCode::new(),
    )
    .unwrap_err()
    .to_string();
    for expected in [
        "there's no tag called `effect.unknown` (in `bad`)",
        "there's no curve called `nope` (in `bad`)",
        "there's no execution called `nowhere` (in `bad`)",
        "there's no effect called `missing` (in `bad`)",
        "there's no attribute called `focus` (in `bad`)",
        "an instant effect with granted tags: it doesn't last (in `bad`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}
