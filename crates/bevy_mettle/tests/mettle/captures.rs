//! What effects capture: magnitudes as expressions, the attributes they read
//! (snapshot or live), specs snapshotted as they're made, what code declares
//! it captures, and what content can't.

use std::sync::{Arc, Mutex};

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::effect::data::MagnitudeDef;
use bevy_mettle::prelude::*;

use crate::common::{
    AttackSpeed, Damage, FIRE, Life, Mana, Strength, actor, attributes, close, get, own, set_base,
};

#[derive(Effect)]
struct Blast;

#[derive(Effect)]
struct Might;

#[derive(Effect)]
struct Aura;

#[derive(Effect)]
struct Mirror;

/// The tags the effects name.
const TAGS: &[&str] = &["damage", "damage.elemental", FIRE, "state", BURNING];

/// A target burning.
const BURNING: &str = "state.burning";

fn curves() -> CurveRegistry {
    CurveRegistry::build(&[
        CurveData {
            name: Key::from_static("growth"),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 1.0), (11.0, 11.0)],
        },
        // Diminishing returns: 1 for 1 up to 10, then 1 for 2.
        CurveData {
            name: Key::from_static("softcap"),
            interpolation: Interpolation::Linear,
            keys: vec![(0.0, 0.0), (10.0, 10.0), (30.0, 20.0)],
        },
    ])
    .unwrap()
}

/// What effects are checked against: what `world` has.
fn scope(world: &World) -> EffectScope<'_> {
    EffectScope {
        tags: world.resource(),
        curves: world.resource(),
        attributes: world.resource(),
        executions: world.resource(),
        calculations: world.resource(),
        routes: world.resource(),
    }
}

/// An app with the attributes, tags and curves, what `code` adds, and
/// `effects` checked into its registry.
fn app(effects: &[EffectData], code: impl FnOnce(&mut App)) -> TestApp {
    TestApp::builder()
        .with(attributes)
        .plugins(EffectPlugin)
        .with(|app| {
            app.insert_resource(
                TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap(),
            )
            .insert_resource(curves());
            code(app);
            let world = app.world();
            let registry = world
                .resource::<EffectRegistry>()
                .rebuild(effects, scope(world))
                .unwrap();
            app.insert_resource(registry);
        })
        .build()
}

/// What checking `effects` in an app with `code` rejects, in words.
fn errors(effects: &[EffectData], code: impl FnOnce(&mut App)) -> String {
    let mut app = App::new();
    attributes(&mut app);
    app.init_resource::<ExecutionRegistry>()
        .init_resource::<CalculationRegistry>()
        .init_resource::<RouteRegistry>()
        .init_resource::<EffectRegistry>()
        .insert_resource(TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap())
        .insert_resource(curves());
    code(&mut app);
    let world = app.world();
    world
        .resource::<EffectRegistry>()
        .rebuild(effects, scope(world))
        .unwrap_err()
        .to_string()
}

/// `effect` adding `value` to the target's `A`.
fn adding<A: Attribute>(effect: EffectData, value: impl Into<Magnitude>) -> EffectData {
    effect.modifier(EffectModifier::new::<A>(Operation::Flat, value))
}

fn apply(app: &mut TestApp, target: Entity, spec: impl Into<EffectSpec>) {
    app.world_mut().apply_effect(target, spec);
    app.tick();
}

/// `actor`'s own modifiers, replaced by `modifiers`.
fn modify(app: &mut TestApp, actor: Entity, modifiers: impl IntoIterator<Item = ModifierSpec>) {
    app.world_mut().entity_mut(actor).modifiers(modifiers);
    app.tick();
}

/// The checked magnitude of `effect`'s first modifier.
fn checked(effect: EffectData) -> MagnitudeDef {
    let app = app(&[effect], |_| {});
    let registry = app.resource::<EffectRegistry>();
    let (_, def) = registry.iter().next().expect("the effect");
    def.modifiers[0].value.clone()
}

#[test]
fn a_scalable_float_is_a_value_times_a_curve_at_the_level() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::scaled(2.0, "growth"),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    // 2 × the curve at 3.
    apply(&mut app, npc, Blast.level(3.0));
    assert!(close(get::<Damage>(&app, npc), 16.0));
}

#[test]
fn a_curve_at_an_attribute_gives_diminishing_returns() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::curve_at("softcap", Magnitude::source::<Strength>()),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let weak = actor(&mut app, []);
    let strong = actor(&mut app, []);
    set_base::<Strength>(&mut app, strong, 30.0);
    app.tick();
    apply(&mut app, npc, Blast.source(weak));
    assert!(close(get::<Damage>(&app, npc), 20.0), "10 strength: 10");
    apply(&mut app, npc, Blast.source(strong));
    assert!(
        close(get::<Damage>(&app, npc), 40.0),
        "30 strength: 20, not 30: {}",
        get::<Damage>(&app, npc)
    );
}

#[test]
fn sums_products_and_clamps_compose() {
    // Half the caster's strength, plus the level, kept within 1 and 8.
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>()
            .times(0.5)
            .plus(Magnitude::Level)
            .clamp(Some(1.0), Some(8.0)),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    apply(&mut app, npc, Blast.source(caster).level(2.0));
    assert!(close(get::<Damage>(&app, npc), 17.0), "5 + 2");
    apply(&mut app, npc, Blast.source(caster).level(5.0));
    assert!(close(get::<Damage>(&app, npc), 25.0), "5 + 5, at most 8");
    set_base::<Strength>(&mut app, caster, 0.0);
    app.tick();
    apply(&mut app, npc, Blast.source(caster).level(0.0));
    assert!(close(get::<Damage>(&app, npc), 26.0), "0 + 0, at least 1");
}

#[test]
fn the_level_is_a_magnitude() {
    let blast = adding::<Damage>(EffectData::instant(Key::of::<Blast>()), Magnitude::Level);
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    apply(&mut app, npc, Blast.level(4.0));
    assert!(close(get::<Damage>(&app, npc), 14.0));
}

#[test]
fn constants_are_folded_as_theyre_checked() {
    let blast = |value: Magnitude| adding::<Damage>(EffectData::instant(Key::of::<Blast>()), value);
    assert_eq!(
        checked(blast(Magnitude::Product(vec![
            Magnitude::Value(2.0),
            Magnitude::Value(5.0),
        ]))),
        MagnitudeDef::Value(10.0)
    );
    assert_eq!(
        checked(blast(Magnitude::Sum(vec![
            Magnitude::Value(1.0),
            Magnitude::Level,
            Magnitude::Value(2.0),
        ]))),
        MagnitudeDef::Sum(vec![MagnitudeDef::Value(3.0), MagnitudeDef::Level]),
        "the constants of a sum with more, folded into one"
    );
    assert_eq!(
        checked(blast(Magnitude::Product(vec![
            Magnitude::Level,
            Magnitude::Value(1.0),
        ]))),
        MagnitudeDef::Level,
        "a product by 1 is what's left"
    );
    assert_eq!(
        checked(blast(Magnitude::from(12.0).clamp(None, Some(8.0)))),
        MagnitudeDef::Value(8.0),
        "a constant clamped"
    );
    assert_eq!(
        checked(blast(
            Magnitude::Sum(vec![Magnitude::Value(4.0), Magnitude::Value(-2.0)])
                .times(3.0)
                .clamp(Some(0.0), None)
        )),
        MagnitudeDef::Value(6.0),
        "nested"
    );
}

#[test]
fn durations_and_periods_are_expressions() {
    // Lasts twice the level; executes every quarter of the caster's cooldown.
    let blast = EffectData::lasting(Key::of::<Blast>(), Magnitude::Level.times(2.0))
        .period(
            Magnitude::source::<crate::common::Cooldown>().times(0.25),
            false,
        )
        .modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0));
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    app.world_mut()
        .apply_effect(npc, Blast.source(caster).level(1.5));
    let effect = app
        .entity(npc)
        .get::<ActiveEffects>()
        .iter()
        .next()
        .unwrap();
    let remaining = app.entity(effect).get::<ActiveEffect>().remaining();
    assert!(
        remaining.is_some_and(|left| close(left, 3.0)),
        "{remaining:?}"
    );
    // A cooldown of 4: every second, for 3.
    app.secs(1.0);
    assert!(
        close(get::<Life>(&app, npc), 49.0),
        "{}",
        get::<Life>(&app, npc)
    );
    app.secs(2.5);
    assert!(
        close(get::<Life>(&app, npc), 47.0),
        "{}",
        get::<Life>(&app, npc)
    );
    assert!(
        app.entity(npc)
            .try_get::<ActiveEffects>()
            .is_none_or(|effects| effects.iter().count() == 0),
        "run out"
    );
}

#[test]
fn a_read_takes_the_current_value_the_base_or_the_bonus() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>(),
    )
    .modifier(EffectModifier::new::<Mana>(
        Operation::Flat,
        Magnitude::source::<Strength>().base(),
    ))
    .modifier(EffectModifier::new::<AttackSpeed>(
        Operation::Flat,
        Magnitude::source::<Strength>().bonus(),
    ));
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, [ModifierSpec::flat::<Strength>(5.0)]);
    apply(&mut app, npc, Blast.source(caster));
    assert!(close(get::<Damage>(&app, npc), 25.0), "current: 15");
    assert!(close(get::<Mana>(&app, npc), 40.0), "base: 10");
    assert!(close(get::<AttackSpeed>(&app, npc), 6.0), "bonus: 5");
}

#[test]
fn a_tagged_read_counts_the_modifiers_that_count_in_its_tags() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>(),
    )
    .modifier(EffectModifier::new::<Mana>(
        Operation::Flat,
        Magnitude::source::<Strength>().tagged(FIRE),
    ))
    .modifier(EffectModifier::new::<AttackSpeed>(
        Operation::Flat,
        Magnitude::source::<Strength>().tagged(FIRE).bonus(),
    ));
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(
        &mut app,
        [ModifierSpec::flat::<Strength>(7.0).read_tagged(FIRE)],
    );
    apply(&mut app, npc, Blast.source(caster));
    assert!(close(get::<Damage>(&app, npc), 20.0), "untagged: 10");
    assert!(close(get::<Mana>(&app, npc), 47.0), "in fire: 17");
    assert!(
        close(get::<AttackSpeed>(&app, npc), 8.0),
        "its bonus in fire: 7"
    );
}

#[test]
fn a_capture_is_read_as_the_hit_its_part_of() {
    // A fire hit, reading its caster's strength with nothing more said.
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()).tag(FIRE),
        Magnitude::source::<Strength>(),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let burning = actor(&mut app, []);
    own(&mut app, burning, BURNING, true);
    let caster = actor(
        &mut app,
        [
            // +7 with fire hits, +5 against burning targets.
            ModifierSpec::flat::<Strength>(7.0).read_tagged(FIRE),
            ModifierSpec::flat::<Strength>(5.0).condition(Condition::target_has(BURNING)),
        ],
    );
    app.tick();
    apply(&mut app, npc, Blast.source(caster));
    assert!(
        close(get::<Damage>(&app, npc), 27.0),
        "its fire bonus counts for a fire hit: 17"
    );
    apply(&mut app, burning, Blast.source(caster));
    assert!(
        close(get::<Damage>(&app, burning), 32.0),
        "and against a burning target: 22"
    );
    // The same as the spec's made: the target's not known yet.
    let made = app.world().snapshot(Blast.source(caster));
    assert_eq!(
        made.snapshot.first().map(|taken| taken.amount),
        Some(17.0),
        "its fire bonus, not the burning one"
    );
}

/// An app with `might`, snapshotting the caster's strength, and `aura`,
/// reading it live; and an npc and a caster.
fn snapshot_and_live(from: fn() -> AttributeRead) -> (TestApp, Entity, Entity) {
    let might = adding::<Damage>(EffectData::infinite(Key::of::<Might>()), from());
    let aura = adding::<Mana>(EffectData::infinite(Key::of::<Aura>()), from().live());
    let mut app = app(&[might, aura], |app| {
        app.add_effect::<Might>().add_effect::<Aura>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    (app, npc, caster)
}

#[test]
fn a_snapshot_keeps_its_value_and_a_live_read_follows_the_source() {
    let (mut app, npc, caster) = snapshot_and_live(Magnitude::source::<Strength>);
    apply(&mut app, npc, Might.source(caster));
    apply(&mut app, npc, Aura.source(caster));
    assert!(close(get::<Damage>(&app, npc), 20.0));
    assert!(close(get::<Mana>(&app, npc), 40.0));
    set_base::<Strength>(&mut app, caster, 25.0);
    app.tick(); // followed in the same step
    assert!(close(get::<Damage>(&app, npc), 20.0), "kept: 10");
    assert!(
        close(get::<Mana>(&app, npc), 55.0),
        "followed: 25, {}",
        get::<Mana>(&app, npc)
    );
    // And back down, by a modifier this time.
    modify(&mut app, caster, [ModifierSpec::flat::<Strength>(-20.0)]);
    app.tick(); // followed in the same step
    assert!(close(get::<Damage>(&app, npc), 20.0));
    assert!(close(get::<Mana>(&app, npc), 35.0), "followed: 5");
}

#[test]
fn a_snapshot_keeps_its_value_and_a_live_read_follows_the_target() {
    let (mut app, npc, _) = snapshot_and_live(Magnitude::target::<Strength>);
    apply(&mut app, npc, Might);
    apply(&mut app, npc, Aura);
    assert!(close(get::<Damage>(&app, npc), 20.0));
    assert!(close(get::<Mana>(&app, npc), 40.0));
    set_base::<Strength>(&mut app, npc, 25.0);
    app.tick(); // followed in the same step
    assert!(close(get::<Damage>(&app, npc), 20.0), "kept: 10");
    assert!(
        close(get::<Mana>(&app, npc), 55.0),
        "followed: 25, {}",
        get::<Mana>(&app, npc)
    );
}

#[test]
fn a_spec_snapshotted_as_its_made_applies_with_its_snapshot() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>(),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    let made = app.world().snapshot(Blast.source(caster));
    assert_eq!(made.snapshot.len(), 1, "{:?}", made.snapshot);
    assert!(close(made.snapshot[0].amount, 10.0));
    // The caster's own: never snapshotted, so captured as it's applied.
    let unmade = Blast.spec().source(caster);
    assert_eq!(unmade.snapshot, []);
    set_base::<Strength>(&mut app, caster, 30.0);
    app.tick();
    apply(&mut app, npc, made);
    assert!(close(get::<Damage>(&app, npc), 20.0), "as it was made: 10");
    apply(&mut app, npc, unmade);
    assert!(close(get::<Damage>(&app, npc), 50.0), "as it's applied: 30");
}

#[test]
fn a_system_snapshots_a_spec_through_snapshots() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>().bonus(),
    )
    .modifier(EffectModifier::new::<Mana>(
        Operation::Flat,
        Magnitude::target::<Strength>(),
    ));
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, [ModifierSpec::flat::<Strength>(4.0)]);
    let made = app
        .world_mut()
        .run_system_once(move |snapshots: Snapshots| snapshots.take(Blast.source(caster)))
        .unwrap();
    assert_eq!(made, app.world().snapshot(Blast.source(caster)), "alike");
    assert_eq!(made.snapshot.len(), 1, "the source's alone");
    modify(&mut app, caster, []);
    set_base::<Strength>(&mut app, npc, 7.0);
    app.tick();
    apply(&mut app, npc, made);
    assert!(
        close(get::<Damage>(&app, npc), 14.0),
        "its bonus as made: 4"
    );
    assert!(
        close(get::<Mana>(&app, npc), 37.0),
        "the target's as applied: 7"
    );
}

#[test]
fn a_snapshotted_spec_outlives_its_source() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>(),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    set_base::<Strength>(&mut app, caster, 12.0);
    app.tick();
    let made = app.world().snapshot(Blast.source(caster));
    let unmade = Blast.spec().source(caster);
    app.entity_mut(caster).despawn();
    app.tick();
    apply(&mut app, npc, made);
    assert!(close(get::<Damage>(&app, npc), 22.0), "its snapshot: 12");
    apply(&mut app, npc, unmade);
    assert!(close(get::<Damage>(&app, npc), 22.0), "nothing to read: 0");
}

#[test]
fn what_a_snapshot_cant_read_yet_is_captured_as_it_applies() {
    let blast = adding::<Damage>(
        EffectData::instant(Key::of::<Blast>()),
        Magnitude::source::<Strength>(),
    );
    let mut app = app(&[blast], |app| {
        app.add_effect::<Blast>();
    });
    let npc = actor(&mut app, []);
    // A caster with no strength yet: nothing to read.
    let caster = app.spawn(OwnedTags::default());
    app.tick();
    let made = app.world().snapshot(Blast.source(caster));
    assert!(
        made.snapshot.is_empty(),
        "nothing taken: {:?}",
        made.snapshot
    );
    app.entity_mut(caster).insert(Base::<Strength>::new(12.0));
    app.tick();
    apply(&mut app, npc, made);
    assert!(
        close(get::<Damage>(&app, npc), 22.0),
        "captured as it applied: 12"
    );
}

#[test]
fn content_rejects_magnitudes_that_cant_be() {
    let blast =
        |name: &'static str, value: Magnitude| adding::<Damage>(EffectData::infinite(name), value);
    let errors = errors(
        &[
            blast("sum", Magnitude::Sum(Vec::new())),
            blast("product", Magnitude::Product(Vec::new())),
            blast("clamp", Magnitude::Level.clamp(Some(5.0), Some(1.0))),
            blast("nan", Magnitude::Value(f32::NAN)),
            blast(
                "infinite",
                Magnitude::Level.clamp(None, Some(f32::INFINITY)),
            ),
            adding::<Strength>(
                EffectData::infinite("itself"),
                Magnitude::target::<Strength>().live(),
            ),
        ],
        |_| {},
    );
    for expected in [
        "a sum of nothing (in `sum`)",
        "a product of nothing (in `product`)",
        "a clamp to at least 5 and at most 1 (in `clamp`)",
        "a value of NaN (in `nan`)",
        "a most of inf (in `infinite`)",
        "a live read of the target's `strength`, which it changes: it would follow itself (in \
         `itself`)",
    ] {
        assert!(errors.contains(expected), "{expected} not in: {errors}");
    }
}

#[test]
fn content_accepts_reads_that_dont_follow_themselves() {
    let app = app(
        &[
            // Snapshotted: read once.
            adding::<Strength>(
                EffectData::infinite(Key::of::<Might>()),
                Magnitude::target::<Strength>(),
            ),
            // The source's: not what it changes.
            adding::<Strength>(
                EffectData::infinite(Key::of::<Aura>()),
                Magnitude::source::<Strength>().live(),
            ),
            // Instant: nothing to follow.
            adding::<Strength>(
                EffectData::instant(Key::of::<Blast>()),
                Magnitude::target::<Strength>().live(),
            ),
        ],
        |_| {},
    );
    assert_eq!(app.resource::<EffectRegistry>().iter().count(), 3);
}

/// What an execution or a calculation read, each time it ran.
#[derive(Clone, Default)]
struct Reads(Arc<Mutex<Vec<[Option<f32>; 3]>>>);

impl Reads {
    fn note(&self, read: [Option<f32>; 3]) {
        self.0.lock().unwrap().push(read);
    }

    /// What was read since last asked.
    fn take(&self) -> Vec<[Option<f32>; 3]> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

/// Notes the caster's strength, snapshotted, the target's damage, live, and
/// the caster's mana, which it doesn't declare.
#[derive(Execution)]
#[execution(source(Strength), target(Damage, live))]
struct Siphon(Reads);

impl ExecutionFn for Siphon {
    fn execute(&self, cx: &ExecutionContext) -> Vec<BaseChange> {
        self.0.note([
            cx.captured::<Strength>(Side::Source),
            cx.captured::<Damage>(Side::Target),
            cx.captured::<Mana>(Side::Source),
        ]);
        assert_eq!(
            cx.captured::<Strength>(Side::Target),
            None,
            "the source's declared, not the target's"
        );
        Vec::new()
    }
}

#[test]
fn an_execution_reads_what_it_declares_it_captures() {
    let reads = Reads::default();
    let siphon = EffectData::lasting(Key::of::<Blast>(), 10.0)
        .period(1.0, false)
        .execution(Key::of::<Siphon>());
    let mut app = app(&[siphon], |app| {
        app.add_effect::<Blast>()
            .add_execution(Siphon(reads.clone()));
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    apply(&mut app, npc, Blast.source(caster));
    app.secs(1.0);
    assert_eq!(reads.take(), [[Some(10.0), Some(10.0), None]]);
    set_base::<Strength>(&mut app, caster, 30.0);
    set_base::<Damage>(&mut app, npc, 40.0);
    app.secs(1.0);
    assert_eq!(
        reads.take(),
        [[Some(10.0), Some(40.0), None]],
        "the strength snapshotted, the damage followed"
    );
}

/// The caster's strength, live.
#[derive(Calculation)]
#[calculation(source(Strength, live))]
struct Following(Reads);

impl CalculationFn for Following {
    fn calculate(&self, cx: &ExecutionContext) -> f32 {
        let read = [
            cx.captured::<Strength>(Side::Source),
            cx.captured::<Strength>(Side::Target),
            cx.captured::<Mana>(Side::Source),
        ];
        self.0.note(read);
        read[0].unwrap_or(-100.0)
    }
}

/// The caster's strength, snapshotted.
#[derive(Calculation)]
#[calculation(source(Strength))]
struct Kept;

impl CalculationFn for Kept {
    fn calculate(&self, cx: &ExecutionContext) -> f32 {
        cx.captured::<Strength>(Side::Source).unwrap_or(-100.0)
    }
}

#[test]
fn a_calculation_reads_what_it_declares_it_captures() {
    let reads = Reads::default();
    let might = adding::<Damage>(
        EffectData::infinite(Key::of::<Might>()),
        Magnitude::Custom(Key::of::<Kept>()),
    );
    let aura = adding::<Mana>(
        EffectData::infinite(Key::of::<Aura>()),
        Magnitude::Custom(Key::of::<Following>()),
    );
    let mut app = app(&[might, aura], |app| {
        app.add_effect::<Might>()
            .add_effect::<Aura>()
            .add_calculation(Kept)
            .add_calculation(Following(reads.clone()));
    });
    let npc = actor(&mut app, []);
    let caster = actor(&mut app, []);
    apply(&mut app, npc, Might.source(caster));
    apply(&mut app, npc, Aura.source(caster));
    assert!(close(get::<Damage>(&app, npc), 20.0));
    assert!(close(get::<Mana>(&app, npc), 40.0));
    let read = reads.take();
    assert_ne!(read.len(), 0, "it ran");
    assert!(
        read.iter().all(|read| read[1..] == [None, None]),
        "only what it declares: {read:?}"
    );
    set_base::<Strength>(&mut app, caster, 25.0);
    app.tick(); // followed in the same step
    assert!(close(get::<Damage>(&app, npc), 20.0), "kept: 10");
    assert!(
        close(get::<Mana>(&app, npc), 55.0),
        "followed: 25, {}",
        get::<Mana>(&app, npc)
    );
}

#[test]
fn a_live_read_follows_what_the_hit_owns() {
    let (mut app, npc, caster) = snapshot_and_live(Magnitude::source::<Strength>);
    modify(
        &mut app,
        caster,
        [ModifierSpec::flat::<Strength>(5.0).condition(Condition::target_has(BURNING))],
    );
    app.tick();
    app.world_mut().apply_effect(npc, Aura.source(caster));
    app.tick();
    assert!(close(get::<Mana>(&app, npc), 40.0), "not burning: 10");
    // The target burns: nothing of the caster's changed, but its read did.
    own(&mut app, npc, BURNING, true);
    app.tick();
    assert!(
        close(get::<Mana>(&app, npc), 45.0),
        "against a burning target: 15, {}",
        get::<Mana>(&app, npc)
    );
    own(&mut app, npc, BURNING, false);
    app.tick();
    assert!(close(get::<Mana>(&app, npc), 40.0), "and back: 10");
}

/// What the npc's mana was told it changed to, each time.
#[derive(Resource, Default)]
struct Told(Vec<f32>);

#[test]
fn a_live_read_is_followed_and_published_once_in_the_step_it_moved() {
    let (mut app, npc, caster) = snapshot_and_live(Magnitude::source::<Strength>);
    app.world_mut().apply_effect(npc, Aura.source(caster));
    app.tick();
    app.init_resource::<Told>();
    app.entity_mut(npc).observe(
        |changed: On<AttributeChanged<Mana>>, mut told: ResMut<Told>| {
            told.0.push(changed.new);
        },
    );
    set_base::<Strength>(&mut app, caster, 25.0);
    app.tick();
    assert!(close(get::<Strength>(&app, caster), 25.0));
    assert!(
        close(get::<Mana>(&app, npc), 55.0),
        "followed in the same step: {}",
        get::<Mana>(&app, npc)
    );
    let told = &app.resource::<Told>().0;
    assert_eq!(told.len(), 1, "told once: {told:?}");
    assert!(close(told[0], 55.0), "settled: {told:?}");
}

#[test]
fn effects_reading_each_other_live_settle() {
    // Each adds half the other's strength to its own: both come to 20.
    let mirror = adding::<Strength>(
        EffectData::infinite(Key::of::<Mirror>()),
        Magnitude::source::<Strength>().live().times(0.5),
    );
    let mut app = app(&[mirror], |app| {
        app.add_effect::<Mirror>();
    });
    let a = actor(&mut app, []);
    let b = actor(&mut app, []);
    app.world_mut().apply_effect(a, Mirror.source(b));
    app.world_mut().apply_effect(b, Mirror.source(a));
    // Each step, they follow each other round after round, as many rounds as
    // settling takes, until what they'd change is too small for an `f32` to
    // hold: a few steps, as each step settles a bounded number of rounds.
    let strengths = |app: &TestApp| {
        (
            get::<Strength>(app, a).to_bits(),
            get::<Strength>(app, b).to_bits(),
        )
    };
    let mut ticks = 0;
    let mut quiet = 0;
    let mut last = strengths(&app);
    while quiet < 10 {
        app.tick();
        ticks += 1;
        assert!(ticks < 200, "still re-counting after {ticks} ticks");
        let now = strengths(&app);
        if now == last {
            quiet += 1;
        } else {
            assert_eq!(quiet, 0, "re-counting after it settled, at {ticks}");
        }
        last = now;
    }
    let settling = ticks - quiet;
    assert!(
        (1..=4).contains(&settling),
        "settled in a few steps, many rounds each: {settling}"
    );
    let settled = (get::<Strength>(&app, a), get::<Strength>(&app, b));
    assert!(
        close(settled.0, 20.0) && close(settled.1, 20.0),
        "{settled:?}"
    );
    app.ticks(10);
    assert_eq!(
        (get::<Strength>(&app, a), get::<Strength>(&app, b)),
        settled,
        "still"
    );
}
