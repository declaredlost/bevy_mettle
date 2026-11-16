//! Effects: applied, lasting, periodic, stacked, inhibited, removed, chained,
//! checked and reloaded.

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::MettleContentPlugin;
use bevy_mettle::core::Error;
use bevy_mettle::effect::code::{CalculationRegistry, ExecutionRegistry, RouteRegistry};
use bevy_mettle::effect::context::ContextDataRegistry;
use bevy_mettle::effect::{EffectScope, EffectSpec};
use bevy_mettle::prelude::*;
use serde::{Deserialize, Serialize};

use crate::common::{
    AttackSpeed, Damage, FIRE, Life, Resistance, Strength, actor, attributes, close, get, own,
};

/// The strike's damage: worked out from both sides, as it captures them.
#[derive(Execution)]
#[execution(
    name = "strike",
    source(Damage),
    target(Resistance, tagged("damage.elemental.fire"))
)]
struct StrikeDamage;

impl ExecutionFn for StrikeDamage {
    fn execute(&self, context: &ExecutionContext) -> Vec<BaseChange> {
        let damage = context.captured::<Damage>(Side::Source).unwrap_or_default();
        let resist = context
            .captured::<Resistance>(Side::Target)
            .unwrap_or_default();
        vec![BaseChange::of::<Life>(
            Operation::Flat,
            -damage * (1.0 - resist),
        )]
    }
}

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

/// Damage by level: 10 at 1, 110 at 11.
#[derive(ScalingCurve)]
#[curve(name = "damage", keys = [(1.0, 10.0), (11.0, 110.0)])]
struct DamageCurve;

/// How hard an effect applied with it hurts.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Hurt {
    amount: f32,
}

/// The tags the effects name.
const TAGS: &[&str] = &[
    "damage.elemental.fire",
    "effect.affliction.burning",
    "effect.affliction.poison",
    "effect.buff",
    "state.burning",
    "state.wet",
    "state.enraged",
    "immune.fire",
];

/// Adds the tags the effects name, and every tag above them, as code's.
fn tags(app: &mut App) {
    let mut registry = app.world_mut().get_resource_or_init::<TagRegistry>();
    for &name in TAGS {
        registry.add_code(TagKey::named(name));
    }
}

/// What effects are checked against outside an app: the tags, the curve,
/// the attributes and the executions, built by hand.
struct Checks {
    tags: TagRegistry,
    curves: CurveRegistry,
    attributes: AttributeRegistry,
    executions: ExecutionRegistry,
    calculations: CalculationRegistry,
    routes: RouteRegistry,
    context: ContextDataRegistry,
}

impl Checks {
    /// The tags, the curve and the attributes, and `executions`.
    fn new(executions: ExecutionRegistry) -> Self {
        let mut app = App::new();
        attributes(&mut app);
        tags(&mut app);
        app.add_curve::<DamageCurve>();
        let world = app.world();
        Self {
            tags: world.resource::<TagRegistry>().rebuild([]).unwrap(),
            curves: world.resource::<CurveRegistry>().rebuild([]).unwrap(),
            attributes: world.resource::<AttributeRegistry>().clone(),
            executions,
            calculations: CalculationRegistry::default(),
            routes: RouteRegistry::default(),
            context: ContextDataRegistry::default(),
        }
    }

    fn scope(&self) -> EffectScope<'_> {
        EffectScope {
            tags: &self.tags,
            curves: &self.curves,
            attributes: &self.attributes,
            executions: &self.executions,
            calculations: &self.calculations,
            routes: &self.routes,
            context: &self.context,
        }
    }
}

/// An app with attributes and effects, the tags, the curve and [`Hurt`], and
/// what `code` adds: loaded as it starts.
fn app(code: impl FnOnce(&mut App)) -> TestApp {
    TestApp::builder()
        .plugins((EffectPlugin, MettleContentPlugin))
        .with(attributes)
        .with(|app| {
            tags(app);
            app.add_curve::<DamageCurve>().register_type::<Hurt>();
            code(app);
        })
        .build()
}

fn apply(app: &mut TestApp, target: Entity, spec: impl Into<EffectSpec>) {
    app.world_mut().apply_effect(target, spec);
    app.tick();
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
    let applied = app
        .read_triggered::<EffectApplied>()
        .into_iter()
        .map(|event| format!("applied {}", event.key));
    let blocked = app
        .read_triggered::<EffectBlocked>()
        .into_iter()
        .map(|event| format!("blocked {} {:?}", event.key, event.reason));
    let stacked = app
        .read_triggered::<EffectStacked>()
        .into_iter()
        .map(|event| format!("stacked {} {}", event.key, event.stacks));
    let removed = app
        .read_triggered::<EffectRemoved>()
        .into_iter()
        .map(|event| format!("removed {} {}", event.key, event.expired));
    applied
        .chain(blocked)
        .chain(stacked)
        .chain(removed)
        .collect()
}

#[test]
fn an_effect_despawned_by_the_game_is_told_and_chained_as_removed() {
    #[derive(Effect)]
    #[effect(infinite, owner_tag = "state.burning", flat::<Damage>(5.0))]
    #[effect(on_remove::<Singed>)]
    struct Burning;

    #[derive(Effect)]
    #[effect(flat::<Life>(-10.0))]
    struct Singed;

    let mut app = app(|app| {
        app.add_effects::<(Burning, Singed)>();
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
    #[derive(Effect)]
    #[effect(infinite, owner_tag = "state.burning")]
    struct Burning;

    let mut app = app(|app| {
        app.add_effect::<Burning>();
    });
    watch(&mut app);
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    told(&mut app);
    let effect = effects_on(&app, npc)[0];
    app.world_mut().despawn(effect);
    app.tick();
    assert_eq!(told(&mut app), ["removed burning false"]);
}

#[test]
fn an_actors_effects_despawned_with_it_tell_nothing() {
    #[derive(Effect)]
    #[effect(infinite, on_remove::<Burning>)]
    struct Burning;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(flat::<Life>(-20.0))]
    struct Hit;

    #[derive(Effect)]
    #[effect(flat::<Life>(-100.0))]
    struct Crush;

    let mut app = app(|app| {
        app.add_effects::<(Hit, Crush)>();
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
fn a_lasting_effect_counts_and_tags_its_owner_while_it_lasts() {
    #[derive(Effect)]
    #[effect(lasting = 1.0, owner_tag = "effect.buff")]
    #[effect(modifier(EffectModifier::new::<AttackSpeed>(Operation::Additive, 0.5)))]
    struct Haste;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(lasting = 2.0, period = 0.5, execute_on_apply, flat::<Life>(-1.0))]
    struct Burning;

    let mut app = app(|app| {
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
fn a_requirement_or_a_blocking_effect_blocks_an_effect() {
    #[derive(Effect)]
    #[effect(lasting = 4.0, tag = "effect.affliction.burning")]
    #[effect(require = !Condition::owner_has("state.wet"))]
    struct Burning;

    #[derive(Effect)]
    #[effect(infinite, blocks = "effect.affliction")]
    struct Warded;

    let mut app = app(|app| {
        app.add_effects::<(Burning, Warded)>();
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
    assert_eq!(told(&mut app), ["blocked burning Effect"]);
}

#[test]
fn an_effect_ends_those_tagged_as_it_says() {
    #[derive(Effect)]
    #[effect(lasting = 10.0, tag = "effect.affliction.poison")]
    struct Poison;

    #[derive(Effect)]
    #[effect(ends = "effect.affliction")]
    struct Cure;

    let mut app = app(|app| {
        app.add_effects::<(Poison, Cure)>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Poison);
    assert_eq!(effects_on(&app, npc).len(), 1);
    apply(&mut app, npc, Cure);
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn stacks_count_their_modifiers_up_to_a_limit_then_overflow() {
    #[derive(Effect)]
    #[effect(lasting = 1.0, flat::<Damage>(2.0))]
    #[effect(stacking = Stacking::up_to(3).expire(Expire::RemoveOne).overflow::<Frenzy>())]
    struct Rage;

    #[derive(Effect)]
    #[effect(flat::<Strength>(1.0))]
    struct Frenzy;

    let mut app = app(|app| {
        app.add_effects::<(Rage, Frenzy)>();
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
    #[derive(Effect)]
    #[effect(infinite, flat::<Damage>(5.0), owner_tag = "state.enraged")]
    #[effect(ongoing = !Condition::owner_has("state.wet"))]
    struct Rage;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(infinite, end_if = Condition::owner_has("state.wet"))]
    struct Burning;

    let mut app = app(|app| {
        app.add_effect::<Burning>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Burning);
    own(&mut app, npc, "state.wet", true);
    app.ticks(2);
    assert_eq!(effects_on(&app, npc), []);
}

#[test]
fn magnitudes_come_from_curves_context_attributes_and_code() {
    #[derive(Effect)]
    #[effect(flat::<Damage>(Magnitude::scaled::<DamageCurve>(0.5)))]
    struct Scaled;

    #[derive(Effect)]
    #[effect(flat::<Life>(Magnitude::context::<Hurt>("amount")))]
    struct Wound;

    #[derive(Effect)]
    #[effect(flat::<Damage>(Magnitude::source::<Strength>().times(0.5)))]
    struct Captured;

    #[derive(Effect)]
    #[effect(flat::<Damage>(Magnitude::calculation::<LevelSquared>()))]
    struct Custom;

    let mut app = app(|app| {
        app.add_calculation(LevelSquared)
            .add_effects::<(Scaled, Wound, Captured, Custom)>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    // 0.5 × the curve at 6: 60.
    apply(&mut app, npc, Scaled.level(6.0));
    assert!(close(get::<Damage>(&app, npc), 40.0));
    apply(&mut app, npc, Wound.with(Hurt { amount: -12.0 }));
    assert!(close(get::<Life>(&app, npc), 38.0));
    // Half the caster's strength of 10.
    apply(&mut app, npc, Captured.source(caster));
    assert!(close(get::<Damage>(&app, npc), 45.0));
    apply(&mut app, npc, Custom.level(3.0));
    assert!(close(get::<Damage>(&app, npc), 54.0));
}

#[test]
fn an_execution_works_damage_out_from_both_sides() {
    #[derive(Effect)]
    #[effect(execution::<StrikeDamage>)]
    struct Strike;

    let mut app = app(|app| {
        app.add_effect::<Strike>().add_execution(StrikeDamage);
    });
    let npc = actor(
        &mut app,
        [ModifierSpec::flat::<Resistance>(0.5).read_tagged(FIRE)],
    );
    let caster = actor(&mut app, [ModifierSpec::flat::<Damage>(10.0)]);
    apply(&mut app, npc, Strike.source(caster));
    // 20 damage, half resisted.
    assert!(close(get::<Life>(&app, npc), 40.0));
}

#[test]
fn an_effect_chained_is_applied_as_itself() {
    #[derive(Effect)]
    #[effect(lasting = 2.0, on_apply::<Spark>)]
    struct Fuse;

    #[derive(Effect)]
    #[effect(lasting = 2.0)]
    struct Spark;

    let mut app = app(|app| {
        app.add_effects::<(Fuse, Spark)>();
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
    #[derive(Effect)]
    #[effect(lasting = 0.5, on_apply::<Spark>, on_expire::<Blast>)]
    struct Fuse;

    #[derive(Effect)]
    #[effect(flat::<Life>(-1.0))]
    struct Spark;

    #[derive(Effect)]
    #[effect(flat::<Life>(-10.0))]
    struct Blast;

    let mut app = app(|app| {
        app.add_effects::<(Fuse, Spark, Blast)>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Fuse);
    assert!(close(get::<Life>(&app, npc), 49.0));
    app.secs(0.5);
    assert!(close(get::<Life>(&app, npc), 39.0));
}

#[test]
fn a_source_term_counts_as_the_source_was_when_applied() {
    #[derive(Effect)]
    #[effect(infinite)]
    #[effect(modifier(
        EffectModifier::new::<Damage>(Operation::Flat, 5.0)
            .condition(Condition::source_has("state.enraged"))
    ))]
    struct Rally;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(infinite, tag = "effect.buff")]
    struct Ward;

    let mut app = app(|app| {
        app.add_effect::<Ward>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Ward.spec().activation(Activation::client(7)));
    apply(&mut app, npc, Ward);
    app.world_mut()
        .commands()
        .entity(npc)
        .remove_effects_under(Activation::client(7));
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
fn reloaded_content_gives_active_effects_their_new_definitions() {
    let haste = |speed: f32| {
        EffectData::new("haste")
            .infinite()
            .flat::<AttackSpeed>(speed)
    };
    let mut app = app(|app| {
        app.insert_resource(MettleContent {
            effects: vec![haste(1.0)],
            ..default()
        });
    });
    let npc = actor(&mut app, []);
    let id = app.resource::<EffectRegistry>().id("haste").expect("haste");
    apply(&mut app, npc, EffectSpec::from_id(id));
    assert!(close(get::<AttackSpeed>(&app, npc), 2.0));
    app.resource_mut::<MettleContent>().effects = vec![haste(3.0)];
    app.ticks(2);
    assert!(close(get::<AttackSpeed>(&app, npc), 4.0));
    // Content without haste: its effect is gone with it.
    app.resource_mut::<MettleContent>().effects.clear();
    app.ticks(2);
    assert!(effects_on(&app, npc).is_empty(), "its effect is gone");
}

#[test]
fn an_effect_left_out_is_as_new() {
    use serde_test::{Token, assert_de_tokens};

    assert_de_tokens(
        &EffectData::new("burning").lasting(4.0),
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
    #[derive(Effect)]
    #[effect(lasting = 4.0, tag = "effect.affliction.burning", period = 0.5)]
    #[effect(execute_on_apply, owner_tag = "state.burning")]
    #[effect(flat::<Life>(Magnitude::context::<Hurt>("amount")))]
    #[effect(require = !Condition::owner_has("immune.fire"))]
    #[effect(ongoing = !Condition::owner_has("state.wet"))]
    #[effect(stacking = Stacking::up_to(3).expire(Expire::RemoveOne))]
    struct Burning;

    let app = app(|app| {
        app.add_effect::<Burning>();
    });
    let registry = app.resource::<EffectRegistry>();
    let burning = registry.id_of::<Burning>().expect("burning, numbered");
    assert_eq!(registry.id("burning"), Some(burning));
    assert!(registry.get(burning).is_some());
}

#[test]
fn a_registry_reports_everything_wrong_at_once() {
    /// An execution no app adds.
    #[derive(Execution)]
    struct Nowhere;

    impl ExecutionFn for Nowhere {
        fn execute(&self, _: &ExecutionContext) -> Vec<BaseChange> {
            Vec::new()
        }
    }

    /// An effect no app adds.
    #[derive(Effect)]
    struct Missing;

    /// A curve no app adds.
    #[derive(ScalingCurve)]
    #[curve(key(1.0, 1.0))]
    struct Nope;

    let focus = EffectModifier::named(Key::from_static("focus"), Operation::Flat, 1.0);
    let bad = EffectData::new("bad")
        .modifier(focus)
        .tag("effect.unknown")
        .owner_tag("state.burning")
        .execution::<Nowhere>()
        .on_apply::<Missing>()
        .flat::<Damage>(Magnitude::scaled::<Nope>(1.0));
    let checks = Checks::new(ExecutionRegistry::default());
    let errors = EffectRegistry::build(&[bad], checks.scope())
        .unwrap_err()
        .to_string();
    for expected in [
        "there's no tag called `effect.unknown` (in `bad`)",
        "there's no curve called `nope` (in `bad`)",
        "there's no execution called `nowhere` (in `bad`)",
        "there's no effect called `missing` (in `bad`)",
        "there's no attribute called `focus` (in `bad`)",
        "an instant effect with owner tags: it doesn't last (in `bad`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}

#[test]
fn executions_need_an_instant_effect_or_a_period() {
    let checks = Checks::new(ExecutionRegistry::default().with(Burn));
    let build = |effect: EffectData| EffectRegistry::build(&[effect], checks.scope());
    let errors = build(EffectData::new("smoulder").lasting(4.0).execution::<Burn>())
        .unwrap_err()
        .to_string();
    assert!(
        errors.contains("executions on a lasting effect with no period: it never executes"),
        "{errors}"
    );
    build(EffectData::new("singe").execution::<Burn>()).unwrap();
    build(
        EffectData::new("burn")
            .lasting(4.0)
            .period(1.0)
            .execution::<Burn>(),
    )
    .unwrap();
}

#[test]
fn an_effect_whose_duration_cant_be_isnt_applied() {
    #[derive(Effect)]
    #[effect(lasting = Magnitude::calculation::<Nan>(), flat::<Damage>(5.0))]
    struct Numb;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(lasting = 5.0, flat::<Damage>(Magnitude::context::<Hurt>("amount")))]
    #[effect(stacking = Stacking::up_to(3))]
    struct Rage;

    let mut app = app(|app| {
        app.add_effect::<Rage>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Rage.with(Hurt { amount: 5.0 }));
    apply(&mut app, npc, Rage.with(Hurt { amount: 9.0 }));
    assert_eq!(effects_on(&app, npc).len(), 1, "one effect, two stacks");
    assert!(
        close(get::<Damage>(&app, npc), 20.0),
        "10 and 5 twice, not 5 and 9"
    );
}

#[test]
fn applying_again_at_the_limit_refreshes_the_time() {
    #[derive(Effect)]
    #[effect(lasting = 1.0, flat::<Damage>(2.0), stacking = Stacking::up_to(1))]
    struct Rage;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(infinite, flat::<Damage>(Magnitude::source::<Strength>()))]
    #[effect(stacking = Stacking::up_to(3))]
    struct Might;

    let mut app = app(|app| {
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
    #[derive(Effect)]
    #[effect(lasting = 0.5, on_expire::<Buff>)]
    struct Relay;

    #[derive(Effect)]
    #[effect(lasting = 0.5, flat::<Damage>(4.0), stacking = Stacking::up_to(1))]
    struct Buff;

    let mut app = app(|app| {
        app.add_effects::<(Relay, Buff)>();
    });
    let npc = actor(&mut app, []);
    app.world_mut().apply_effect(npc, Relay);
    app.world_mut().apply_effect(npc, Buff);
    app.secs(0.5);
    app.tick();
    let buffs: Vec<Entity> = effects_on(&app, npc)
        .into_iter()
        .filter(|effect| app.entity(*effect).get::<ActiveEffect>().is::<Buff>())
        .collect();
    assert_eq!(buffs.len(), 1, "applied again as it ran out");
    assert!(close(get::<Damage>(&app, npc), 14.0));
}

#[test]
fn content_defining_an_effect_code_defines_fails_the_check() {
    #[derive(Effect)]
    #[effect(flat::<Life>(-1.0))]
    struct Burning;

    let mut app = App::new();
    attributes(&mut app);
    app.add_effect::<Burning>();
    let checks = Checks::new(ExecutionRegistry::default());
    let checks = checks.scope();
    let code = app.world().resource::<EffectRegistry>();
    let burning = EffectData::new("burning").lasting(1.0);
    let errors = code
        .rebuild(std::slice::from_ref(&burning), checks)
        .unwrap_err();
    let defined = Error::DefinedInCode {
        kind: "effect",
        name: "burning".to_owned(),
    };
    assert!(errors.0.contains(&defined), "{errors}");
    assert!(
        errors
            .to_string()
            .contains("content defines effect `burning`, which code defines"),
        "{errors}"
    );
    // What code doesn't define, content adds; checked afresh, knowing no
    // types, it's content's to define.
    let rebuilt = code
        .rebuild(&[EffectData::new("hit").flat::<Life>(-5.0)], checks)
        .unwrap();
    assert!(rebuilt.id_of::<Burning>().is_some() && rebuilt.id("hit").is_some());
    EffectRegistry::build(&[burning], checks).unwrap();
}
