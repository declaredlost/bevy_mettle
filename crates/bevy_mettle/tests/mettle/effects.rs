//! Effects: applied, lasting, periodic, stacked, inhibited, removed, chained,
//! checked and reloaded.

use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::Error;
use bevy_mettle::core::registry::{EffectId, Name};
use bevy_mettle::prelude::*;

use crate::common::{
    AttackSpeed, Damage, FIRE, Life, Resistance, Strength, actor, attributes, close, get, id, own,
};

/// The effects the tests apply, as code names them.
#[derive(Effect)]
struct Burning;

#[derive(Effect)]
struct Hit;

#[derive(Effect)]
struct Crush;

#[derive(Effect)]
struct Haste;

#[derive(Effect)]
struct Warded;

#[derive(Effect)]
struct Poison;

#[derive(Effect)]
struct Cure;

#[derive(Effect)]
struct Rage;

#[derive(Effect)]
struct Curve;

#[derive(Effect)]
struct Param;

#[derive(Effect)]
struct Captured;

#[derive(Effect)]
struct Custom;

/// The strike, an effect running [`StrikeDamage`].
#[derive(Effect)]
struct Strike;

/// The strike's damage: worked out from both sides.
#[derive(Execution)]
#[execution(name = "strike")]
struct StrikeDamage;

impl ExecutionFn for StrikeDamage {
    fn execute(&self, context: &ExecutionContext) -> Vec<BaseChange> {
        let query = AttributeContext::new();
        let damage = context
            .source()
            .and_then(|source| context.value::<Damage>(source, &query))
            .unwrap_or_default();
        let resist = context
            .value::<Resistance>(context.target, &AttributeContext::new().tag(FIRE))
            .unwrap_or_default();
        vec![BaseChange::new(
            id::<Life>(),
            Operation::Flat,
            -damage * (1.0 - resist),
        )]
    }
}

#[derive(Effect)]
struct Fuse;

#[derive(Effect)]
struct Rally;

#[derive(Effect)]
struct Ward;

#[derive(Effect)]
struct Numb;

#[derive(Effect)]
struct Might;

#[derive(Effect)]
struct Relay;

#[derive(Effect)]
struct Buff;

/// An execution that burns.
#[derive(Execution)]
struct Burn;

impl ExecutionFn for Burn {
    fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
        Vec::new()
    }
}

/// The level, squared.
#[derive(Calculation)]
struct LevelSquared;

impl CalculationFn for LevelSquared {
    fn calculate(&self, context: &ExecutionContext) -> f32 {
        context.level() * context.level()
    }
}

/// A number that isn't one.
#[derive(Calculation)]
struct Nan;

impl CalculationFn for Nan {
    fn calculate(&self, _: &ExecutionContext) -> f32 {
        f32::NAN
    }
}

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

fn tags() -> TagRegistry {
    TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap()
}

fn curves() -> CurveRegistry {
    CurveRegistry::build(&[CurveData {
        name: Name::from_static("damage"),
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

/// What effects are checked against: what `world` has.
fn checks(world: &World) -> EffectChecks<'_> {
    EffectChecks {
        tags: world.resource(),
        curves: world.resource(),
        attributes: world.resource(),
        executions: world.resource(),
        calculations: world.resource(),
        routes: world.resource(),
    }
}

/// `world`'s effect registry, rechecked with `effects`: what a reload does.
fn registry(world: &World, effects: &[EffectData]) -> EffectRegistry {
    world
        .resource::<EffectRegistry>()
        .recheck(effects, checks(world))
        .unwrap()
}

/// An app with attributes and effects, the tags and curves, what `code` adds,
/// and `effects` checked into its registry.
fn app(effects: &[EffectData], code: impl FnOnce(&mut App)) -> TestApp {
    TestApp::builder()
        .with(attributes)
        .plugins(EffectPlugin)
        .with(|app| {
            app.insert_resource(tags()).insert_resource(curves());
            code(app);
            let registry = registry(app.world(), effects);
            app.insert_resource(registry);
        })
        .build()
}

fn apply(app: &mut TestApp, target: Entity, spec: impl Into<EffectSpec>) {
    app.world_mut().apply_effect(target, spec);
    app.tick();
}

fn flat<A: Attribute>(value: f32) -> EffectModifier {
    EffectModifier::new::<A>(Operation::Flat, value)
}

fn effects_on(app: &TestApp, target: Entity) -> Vec<Entity> {
    app.entity(target)
        .try_get::<ActiveEffects>()
        .map(|effects| effects.iter().collect())
        .unwrap_or_default()
}

fn owns(app: &TestApp, target: Entity, tag: &'static str) -> bool {
    app.entity(target).get::<OwnedTags>().contains(tag)
}

/// Records what an effect tells as it comes, stacks, is blocked or goes.
fn watch(app: &mut TestApp) {
    app.watch_triggered::<EffectApplied>();
    app.watch_triggered::<EffectBlocked>();
    app.watch_triggered::<EffectStacked>();
    app.watch_triggered::<EffectRemoved>();
}

/// What was told since last asked, each as it reads: `"applied warded"`.
///
/// By kind, so a test asks after each step to know which came first.
fn told(app: &mut TestApp) -> Vec<String> {
    let registry = app.resource::<EffectRegistry>().clone();
    let name = |id: EffectId| registry.name(id).expect("an effect").to_string();
    let applied = app
        .read_triggered::<EffectApplied>()
        .into_iter()
        .map(|event| format!("applied {}", name(event.id)));
    let blocked = app
        .read_triggered::<EffectBlocked>()
        .into_iter()
        .map(|event| format!("blocked {} {:?}", name(event.id), event.reason));
    let stacked = app
        .read_triggered::<EffectStacked>()
        .into_iter()
        .map(|event| format!("stacked {} {}", name(event.id), event.stacks));
    let removed = app
        .read_triggered::<EffectRemoved>()
        .into_iter()
        .map(|event| format!("removed {} {}", name(event.id), event.expired));
    applied
        .chain(blocked)
        .chain(stacked)
        .chain(removed)
        .collect()
}

#[test]
fn an_effect_despawned_by_the_game_is_told_and_chained_as_removed() {
    let burning = EffectData::infinite("burning")
        .grant("state.burning")
        .modifier(flat::<Damage>(5.0))
        .on_remove("singed");
    let singed = EffectData::instant("singed").modifier(flat::<Life>(-10.0));
    let mut app = app(&[burning, singed], |app| {
        app.add_effect::<Burning>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    assert_eq!(told(&mut app), ["applied burning"]);

    let effect = effects_on(&app, npc)[0];
    app.entity_mut(effect).despawn();
    app.tick();
    assert_eq!(told(&mut app), ["removed burning false"], "told once");
    assert!(!owns(&app, npc, "state.burning"), "its tag taken back");
    assert!(close(get::<Damage>(&app, npc), 10.0), "its modifier gone");
    assert!(close(get::<Life>(&app, npc), 40.0), "chained: singed");
}

#[test]
fn an_effect_removed_by_mettle_is_told_once() {
    let burning = EffectData::infinite("burning").grant("state.burning");
    let mut app = app(&[burning], |app| {
        app.add_effect::<Burning>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    told(&mut app);
    let effect = effects_on(&app, npc)[0];
    app.world_mut().remove_effect(effect);
    app.tick();
    assert_eq!(told(&mut app), ["removed burning false"]);
}

#[test]
fn an_actors_effects_despawned_with_it_tell_nothing() {
    let burning = EffectData::infinite("burning").on_remove("burning");
    let mut app = app(&[burning], |app| {
        app.add_effect::<Burning>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    told(&mut app);
    app.entity_mut(npc).despawn();
    app.tick();
    assert!(told(&mut app).is_empty(), "nothing left to tell");
}

#[test]
fn an_instant_effect_changes_the_base_within_its_limits() {
    let hit = EffectData::instant("hit").modifier(flat::<Life>(-20.0));
    let crush = EffectData::instant("crush").modifier(flat::<Life>(-100.0));
    let mut app = app(&[hit, crush], |app| {
        app.add_effect::<Hit>().add_effect::<Crush>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Hit);
    assert!(close(get::<Life>(&app, npc), 30.0));
    apply(&mut app, npc, Crush);
    let base = app.entity(npc).get::<Base<Life>>().get();
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
    let haste = EffectData::lasting("haste", 1.0)
        .modifier(EffectModifier::new::<AttackSpeed>(Operation::Additive, 0.5))
        .grant("effect.buff");
    let mut app = app(&[haste], |app| {
        app.add_effect::<Haste>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Haste);
    assert!(close(get::<AttackSpeed>(&app, npc), 1.5));
    assert!(owns(&app, npc, "effect.buff"));
    app.secs(1.0);
    assert!(close(get::<AttackSpeed>(&app, npc), 1.0), "run out");
    assert!(!owns(&app, npc, "effect.buff"));
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn a_periodic_effect_executes_each_period_and_on_apply() {
    let burning = EffectData::lasting("burning", 2.0)
        .period(0.5, true)
        .modifier(flat::<Life>(-1.0));
    let mut app = app(&[burning], |app| {
        app.add_effect::<Burning>();
    });
    let npc = actor(&mut app, []);
    app.world_mut().apply_effect(npc, Burning);
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
    let burning = EffectData::lasting("burning", 4.0)
        .tag("effect.affliction.burning")
        .require(!Condition::owner_has("state.wet"));
    let warded = EffectData::infinite("warded").immunity("effect.affliction");
    let mut app = app(&[burning, warded], |app| {
        app.add_effect::<Burning>().add_effect::<Warded>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    own(&mut app, npc, "state.wet", true);
    apply(&mut app, npc, Burning);
    assert_eq!(told(&mut app), ["blocked burning Requirement"]);
    own(&mut app, npc, "state.wet", false);
    apply(&mut app, npc, Warded);
    assert_eq!(told(&mut app), ["applied warded"]);
    apply(&mut app, npc, Burning);
    assert_eq!(told(&mut app), ["blocked burning Immunity"]);
}

#[test]
fn an_effect_removes_those_tagged_as_it_says() {
    let poison = EffectData::lasting("poison", 10.0).tag("effect.affliction.poison");
    let cure = EffectData::instant("cure").removes("effect.affliction");
    let mut app = app(&[poison, cure], |app| {
        app.add_effect::<Poison>().add_effect::<Cure>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Poison);
    assert_eq!(effects_on(&app, npc).len(), 1);
    apply(&mut app, npc, Cure);
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn stacks_count_their_modifiers_up_to_a_limit_then_overflow() {
    let rage = EffectData::lasting("rage", 1.0)
        .modifier(flat::<Damage>(2.0))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 3,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::RemoveOne,
            overflow: vec![Name::from_static("frenzy")],
        });
    let frenzy = EffectData::instant("frenzy").modifier(flat::<Strength>(1.0));
    let mut app = app(&[rage, frenzy], |app| {
        app.add_effect::<Rage>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    let told: Vec<Vec<String>> = (0..4)
        .map(|_| {
            apply(&mut app, npc, Rage);
            told(&mut app)
        })
        .collect();
    assert!(close(get::<Damage>(&app, npc), 16.0), "three stacks");
    assert!(close(get::<Strength>(&app, npc), 11.0), "overflowed once");
    assert_eq!(effects_on(&app, npc).len(), 1);
    assert_eq!(
        told,
        [
            ["applied rage"],
            ["stacked rage 2"],
            ["stacked rage 3"],
            ["blocked rage StackLimit"]
        ]
    );
    app.secs(1.0);
    assert!(close(get::<Damage>(&app, npc), 14.0), "one stack off");
}

#[test]
fn an_effect_is_inhibited_while_its_ongoing_requirement_fails() {
    let rage = EffectData::infinite("rage")
        .modifier(flat::<Damage>(5.0))
        .grant("state.enraged")
        .ongoing(!Condition::owner_has("state.wet"));
    let mut app = app(&[rage], |app| {
        app.add_effect::<Rage>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Rage);
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
    let burning = EffectData::infinite("burning").remove_if(Condition::owner_has("state.wet"));
    let mut app = app(&[burning], |app| {
        app.add_effect::<Burning>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    own(&mut app, npc, "state.wet", true);
    app.ticks(2);
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn magnitudes_come_from_curves_params_attributes_and_code() {
    let curve = EffectData::instant("curve").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::scaled(0.5, "damage"),
    ));
    let param = EffectData::instant("param").modifier(EffectModifier::new::<Life>(
        Operation::Flat,
        Magnitude::param("param.damage"),
    ));
    let captured = EffectData::instant("captured").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::source::<Strength>().times(0.5),
    ));
    let custom = EffectData::instant("custom").modifier(EffectModifier::new::<Damage>(
        Operation::Flat,
        Magnitude::Custom(Name::from_static("level_squared")),
    ));
    let mut app = app(&[curve, param, captured, custom], |app| {
        app.add_calculation(LevelSquared)
            .add_effect::<Curve>()
            .add_effect::<Param>()
            .add_effect::<Captured>()
            .add_effect::<Custom>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    // 0.5 × the curve at 6: 60.
    apply(&mut app, npc, Curve.level(6.0));
    assert!(close(get::<Damage>(&app, npc), 40.0));
    apply(&mut app, npc, Param.param("param.damage", -12.0));
    assert!(close(get::<Life>(&app, npc), 38.0));
    // Half the caster's strength of 10.
    apply(&mut app, npc, Captured.source(caster));
    assert!(close(get::<Damage>(&app, npc), 45.0));
    apply(&mut app, npc, Custom.level(3.0));
    assert!(close(get::<Damage>(&app, npc), 54.0));
}

#[test]
fn an_execution_works_damage_out_from_both_sides() {
    let strike = EffectData::instant("strike").execution("strike");
    let mut app = app(&[strike], |app| {
        app.add_effect::<Strike>().add_execution(StrikeDamage);
    });
    let npc = actor(
        &mut app,
        [Modifier::flat(id::<Resistance>(), 0.5).read_tagged(FIRE)],
    );
    let caster = actor(&mut app, [Modifier::flat(id::<Damage>(), 10.0)]);
    apply(&mut app, npc, Strike.source(caster));
    // 20 damage, half resisted.
    assert!(close(get::<Life>(&app, npc), 40.0));
}

#[test]
fn an_effect_chained_is_applied_as_itself() {
    let fuse = EffectData::lasting("fuse", 2.0).on_apply("spark");
    let spark = EffectData::lasting("spark", 2.0);
    let mut app = app(&[fuse, spark], |app| {
        app.add_effect::<Fuse>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Fuse.level(3.0));
    let registry = app.resource::<EffectRegistry>().clone();
    let effects = effects_on(&app, npc);
    assert_eq!(effects.len(), 2);
    for effect in effects {
        let active = app.entity(effect).get::<ActiveEffect>();
        assert_eq!(
            active.spec().effect_id(&registry),
            Some(active.id()),
            "its spec names it, not what chained it"
        );
        assert!(
            close(active.spec().level, 3.0),
            "at the level it was chained at"
        );
    }
}

#[test]
fn effects_apply_effects_as_they_apply_and_expire() {
    let fuse = EffectData::lasting("fuse", 0.5)
        .on_apply("spark")
        .on_expire("blast");
    let spark = EffectData::instant("spark").modifier(flat::<Life>(-1.0));
    let blast = EffectData::instant("blast").modifier(flat::<Life>(-10.0));
    let mut app = app(&[fuse, spark, blast], |app| {
        app.add_effect::<Fuse>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Fuse);
    assert!(close(get::<Life>(&app, npc), 49.0));
    app.secs(0.5);
    assert!(close(get::<Life>(&app, npc), 39.0));
}

#[test]
fn a_source_term_counts_as_the_source_was_when_applied() {
    let rally = EffectData::infinite("rally")
        .modifier(flat::<Damage>(5.0).condition(Condition::source_has("state.enraged")));
    let mut app = app(&[rally], |app| {
        app.add_effect::<Rally>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    own(&mut app, caster, "state.enraged", true);
    apply(&mut app, npc, Rally.source(caster));
    own(&mut app, caster, "state.enraged", false);
    app.tick();
    assert!(close(get::<Damage>(&app, npc), 15.0), "as it was");
    apply(&mut app, npc, Rally.source(caster));
    assert!(
        close(get::<Damage>(&app, npc), 15.0),
        "a calm one adds nothing"
    );
}

#[test]
fn effects_come_off_by_activation_tag_or_with_their_actor() {
    let ward = EffectData::infinite("ward").tag("effect.buff");
    let mut app = app(&[ward], |app| {
        app.add_effect::<Ward>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Ward.spec().activation(7));
    apply(&mut app, npc, Ward);
    app.world_mut().commands().entity(npc).remove_activation(7);
    app.tick();
    assert_eq!(
        effects_on(&app, npc).len(),
        1,
        "the predicted one taken back"
    );
    let last = effects_on(&app, npc)[0];
    app.entity_mut(npc).despawn();
    assert!(!app.entity(last).is_spawned(), "gone with its actor");
}

#[test]
fn a_new_registry_gives_active_effects_their_new_definitions() {
    let haste = EffectData::infinite("haste").modifier(flat::<AttackSpeed>(1.0));
    let mut app = app(&[haste], |app| {
        app.add_effect::<Haste>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Haste);
    assert!(close(get::<AttackSpeed>(&app, npc), 2.0));
    let faster = EffectData::infinite("haste").modifier(flat::<AttackSpeed>(3.0));
    let reloaded = registry(app.world(), &[faster]);
    app.insert_resource(reloaded);
    app.ticks(2);
    assert!(close(get::<AttackSpeed>(&app, npc), 4.0));
    // Content without haste: checked afresh, as code no longer names it.
    let emptied = EffectRegistry::check(&[], checks(app.world())).unwrap();
    app.insert_resource(emptied);
    app.ticks(2);
    assert!(effects_on(&app, npc).is_empty(), "its effect is gone");
}

#[test]
fn an_effect_left_out_is_as_new() {
    use serde_test::{Token, assert_de_tokens};

    assert_de_tokens(
        &EffectData::lasting("burning", 4.0),
        &[
            Token::Struct {
                name: "EffectData",
                len: 2,
            },
            Token::Str("name"),
            Token::Str("burning"),
            Token::Str("duration"),
            Token::NewtypeVariant {
                name: "EffectDuration",
                variant: "For",
            },
            Token::NewtypeVariant {
                name: "Magnitude",
                variant: "Value",
            },
            Token::F32(4.0),
            Token::StructEnd,
        ],
    );
}

#[test]
fn an_effect_with_everything_checks() {
    let def = EffectData::lasting("burning", 4.0)
        .tag("effect.affliction.burning")
        .period(0.5, true)
        .modifier(EffectModifier::new::<Life>(
            Operation::Flat,
            Magnitude::param("param.damage"),
        ))
        .grant("state.burning")
        .require(!Condition::owner_has("immune.fire"))
        .ongoing(!Condition::owner_has("state.wet"))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 3,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::RemoveOne,
            overflow: Vec::new(),
        });
    let app = app(&[def], |app| {
        app.add_effect::<Burning>();
    });
    let registry = app.resource::<EffectRegistry>();
    let burning = registry.id_of::<Burning>().expect("burning, numbered");
    assert_eq!(registry.id("burning"), Some(burning));
    assert!(registry.get(burning).is_some());
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    let focus = EffectModifier::named(Name::from_static("focus"), Operation::Flat, 1.0);
    let bad = EffectData::instant("bad")
        .modifier(focus)
        .tag("effect.unknown")
        .grant("state.burning")
        .execution("nowhere")
        .on_apply("missing")
        .modifier(EffectModifier::new::<Damage>(
            Operation::Flat,
            Magnitude::scaled(1.0, "nope"),
        ));
    let errors = EffectRegistry::check(
        &[bad],
        EffectChecks {
            tags: &tags(),
            curves: &curves(),
            attributes: &attribute_registry(),
            executions: &ExecutionRegistry::default(),
            calculations: &CalculationRegistry::default(),
            routes: &RouteRegistry::default(),
        },
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

#[test]
fn executions_need_an_instant_effect_or_a_period() {
    let executions = ExecutionRegistry::default().with(Burn);
    let build = |effect: EffectData| {
        EffectRegistry::check(
            &[effect],
            EffectChecks {
                tags: &tags(),
                curves: &curves(),
                attributes: &attribute_registry(),
                executions: &executions,
                calculations: &CalculationRegistry::default(),
                routes: &RouteRegistry::default(),
            },
        )
    };
    let errors = build(EffectData::lasting("smoulder", 4.0).execution("burn"))
        .unwrap_err()
        .to_string();
    assert!(
        errors.contains("executions on a lasting effect with no period: it never executes"),
        "{errors}"
    );
    build(EffectData::instant("singe").execution("burn")).unwrap();
    build(
        EffectData::lasting("burn", 4.0)
            .period(1.0, false)
            .execution("burn"),
    )
    .unwrap();
}

#[test]
fn an_effect_whose_duration_cant_be_isnt_applied() {
    let numb = EffectData::lasting("numb", Magnitude::Custom(Name::from_static("nan")))
        .modifier(flat::<Damage>(5.0));
    let mut app = app(&[numb], |app| {
        app.add_calculation(Nan).add_effect::<Numb>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Numb);
    assert_eq!(effects_on(&app, npc), [], "it'd never run out");
    assert!(close(get::<Damage>(&app, npc), 10.0));
    assert!(told(&mut app).is_empty(), "nothing told");
}

#[test]
fn a_stack_keeps_the_spec_it_was_first_applied_with() {
    let rage = EffectData::lasting("rage", 5.0)
        .modifier(EffectModifier::new::<Damage>(
            Operation::Flat,
            Magnitude::param("param.damage"),
        ))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 3,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::ClearAll,
            overflow: Vec::new(),
        });
    let mut app = app(&[rage], |app| {
        app.add_effect::<Rage>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Rage.param("param.damage", 5.0));
    apply(&mut app, npc, Rage.param("param.damage", 9.0));
    assert_eq!(effects_on(&app, npc).len(), 1, "one effect, two stacks");
    assert!(
        close(get::<Damage>(&app, npc), 20.0),
        "10 and 5 twice, not 5 and 9"
    );
}

#[test]
fn applying_again_at_the_limit_refreshes_the_time() {
    let rage = EffectData::lasting("rage", 1.0)
        .modifier(flat::<Damage>(2.0))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 1,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::ClearAll,
            overflow: Vec::new(),
        });
    let mut app = app(&[rage], |app| {
        app.add_effect::<Rage>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Rage);
    app.secs(0.6);
    apply(&mut app, npc, Rage);
    app.secs(0.6);
    assert_eq!(effects_on(&app, npc).len(), 1, "refreshed at the limit");
    assert!(close(get::<Damage>(&app, npc), 12.0));
}

#[test]
fn an_attribute_magnitude_is_captured_as_the_effect_applies() {
    let might = EffectData::infinite("might")
        .modifier(EffectModifier::new::<Damage>(
            Operation::Flat,
            Magnitude::source::<Strength>(),
        ))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 3,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::ClearAll,
            overflow: Vec::new(),
        });
    let mut app = app(&[might], |app| {
        app.add_effect::<Might>();
    });
    let caster = actor(&mut app, []);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Might.source(caster));
    assert!(close(get::<Damage>(&app, npc), 20.0));
    app.world_mut()
        .get_mut::<Base<Strength>>(caster)
        .unwrap()
        .set(20.0);
    app.tick();
    apply(&mut app, npc, Might.source(caster));
    assert!(
        close(get::<Damage>(&app, npc), 30.0),
        "two stacks of the 10 captured"
    );
}

#[test]
fn an_effect_refreshed_earlier_in_a_tick_doesnt_run_out_in_it() {
    // The relay runs out as the buff does, and applies the buff again as it
    // does: whichever settles first, the buff is there after.
    let relay = EffectData::lasting("relay", 0.5).on_expire("buff");
    let buff = EffectData::lasting("buff", 0.5)
        .modifier(flat::<Damage>(4.0))
        .stacking(Stacking {
            by: StackBy::Target,
            limit: 1,
            reapply: Reapply::Refresh,
            reset_period: false,
            expire: Expire::ClearAll,
            overflow: Vec::new(),
        });
    let mut app = app(&[relay, buff], |app| {
        app.add_effect::<Relay>().add_effect::<Buff>();
    });
    let npc = actor(&mut app, []);
    app.world_mut().apply_effect(npc, Relay);
    app.world_mut().apply_effect(npc, Buff);
    app.secs(0.5);
    app.tick();
    let buff = app
        .resource::<EffectRegistry>()
        .id_of::<Buff>()
        .expect("the buff");
    let buffs: Vec<Entity> = effects_on(&app, npc)
        .into_iter()
        .filter(|effect| app.entity(*effect).get::<ActiveEffect>().id() == buff)
        .collect();
    assert_eq!(buffs.len(), 1, "applied again as it ran out");
    assert!(close(get::<Damage>(&app, npc), 14.0));
}

#[test]
fn code_naming_an_effect_content_lacks_fails_the_check() {
    let mut app = App::new();
    attributes(&mut app);
    app.add_effect::<Burning>();
    let world = app.world();
    let checks = EffectChecks {
        tags: &tags(),
        curves: &curves(),
        attributes: world.resource(),
        executions: &ExecutionRegistry::default(),
        calculations: &CalculationRegistry::default(),
        routes: &RouteRegistry::default(),
    };
    let named = world.resource::<EffectRegistry>();
    let errors = named
        .recheck(&[EffectData::instant("hit")], checks)
        .unwrap_err();
    let missing = Error::NotInContent {
        kind: "effect",
        name: "burning".to_owned(),
    };
    assert!(errors.0.contains(&missing), "{errors}");
    assert!(
        errors
            .to_string()
            .contains("code names effect `burning`, which content doesn't define"),
        "{errors}"
    );
    // Checked afresh, nothing named: it's content's to leave out.
    EffectRegistry::check(&[EffectData::instant("hit")], checks).unwrap();
    named
        .recheck(&[EffectData::instant("burning")], checks)
        .unwrap();
}
