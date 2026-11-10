//! Mettle's hot paths, measured: attributes kept and read, tags and
//! conditions checked, effects applied, routed and ticked, and captures
//! executed and followed, for 1000 actors.
//!
//! `cargo bench -p bevy_mettle --bench mettle`.

use std::hint::black_box;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_mettle::core::condition::Context;
use bevy_mettle::core::registry::AttributeId;
use bevy_mettle::prelude::*;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};

macro_rules! attributes {
    ($($name:ident),*) => {
        $(#[derive(Attribute)] #[attribute(base = 10.0, min = 0.0)] struct $name;)*
        fn add_all(app: &mut App) { $(app.add_attribute::<$name>();)* }
        fn bases() -> impl Bundle { ($(Base::<$name>::default(),)*) }
    };
}
attributes!(A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11);

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxLife;
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0, max = MaxLife)]
struct Life;
#[derive(Attribute)]
#[attribute(min = 0.0)]
struct Shield;
#[derive(Route)]
struct IncomingDamage;

impl RouteFn for IncomingDamage {
    fn route(&self, effect: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
        let shield = effect
            .value::<Shield>(effect.target, &AttributeContext::new())
            .unwrap_or_default();
        let absorbed = damage.min(shield);
        let (Some(shield), Some(life)) = (
            effect.attribute_id::<Shield>(),
            effect.attribute_id::<Life>(),
        ) else {
            return Vec::new();
        };
        vec![
            BaseChange::new(shield, Operation::Flat, -absorbed),
            BaseChange::new(life, Operation::Flat, absorbed - damage),
        ]
    }
}

/// The effects the benches apply, as code names them.
#[derive(Effect)]
struct Hit;
#[derive(Effect)]
struct Routed;
#[derive(Effect)]
struct Struck;
#[derive(Effect)]
struct Burning;
/// What a strike executes.
#[derive(Execution)]
struct Strike;

impl ExecutionFn for Strike {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        effect
            .attribute_id::<Life>()
            .map(|life| BaseChange::new(life, Operation::Flat, -effect.level()))
            .into_iter()
            .collect()
    }
}

/// What a scorch executes: its source's `A0` as it applied, less its
/// target's `A1` as it is now, off its life.
#[derive(Execution)]
#[execution(source(A0), target(A1, live))]
struct Scorch;

impl ExecutionFn for Scorch {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let power = effect.captured::<A0>(Capture::Source).unwrap_or_default();
        let resist = effect.captured::<A1>(Capture::Target).unwrap_or_default();
        effect
            .attribute_id::<Life>()
            .map(|life| BaseChange::new(life, Operation::Flat, (resist - power).min(0.0)))
            .into_iter()
            .collect()
    }
}

/// Effects capturing what they read, as code names them.
#[derive(Effect)]
struct Scorching;
#[derive(Effect)]
struct Aura;

const ACTORS: u16 = 1000;
const FIRE: &str = "damage.elemental.fire";

/// Each actor's own modifiers: 16, a quarter of them scoped to fire reads.
fn modifiers(attributes: &AttributeRegistry, seed: f32) -> Modifiers {
    let mut all = vec![
        Modifier::flat(id::<A0>(attributes), seed),
        Modifier::flat(id::<A1>(attributes), 1.0),
        Modifier::flat(id::<A2>(attributes), 2.0),
        Modifier::flat(id::<A3>(attributes), 3.0),
        Modifier::flat(id::<A4>(attributes), 4.0),
        Modifier::flat(id::<A5>(attributes), 5.0),
        Modifier::flat(id::<A6>(attributes), 6.0),
        Modifier::flat(id::<A7>(attributes), 7.0),
        Modifier::flat(id::<A8>(attributes), 8.0),
        Modifier::flat(id::<A9>(attributes), 9.0),
        Modifier::flat(id::<A10>(attributes), 10.0),
        Modifier::flat(id::<A11>(attributes), 11.0),
    ];
    for _ in 0..4 {
        all.push(Modifier::flat(id::<A3>(attributes), 0.5).read_tagged(FIRE));
    }
    all.into_iter().collect()
}

fn tags() -> TagRegistry {
    let defs: Vec<TagKey> = [
        "damage",
        "damage.elemental",
        "damage.elemental.fire",
        "damage.elemental.cold",
        "damage.physical",
        "state",
        "state.stunned",
        "state.burning",
        "state.wet",
        "state.casting",
        "cooldown",
        "cooldown.fireball",
        "effect",
        "effect.affliction",
        "effect.affliction.burning",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect();
    TagRegistry::build(defs).unwrap()
}

/// `A`'s number in the app `attributes` are.
fn id<A: Attribute>(attributes: &AttributeRegistry) -> AttributeId {
    attributes
        .id_of::<A>()
        .expect("an attribute the benches add")
}

/// An app keeping attributes every update, with `ACTORS` actors.
fn attribute_app() -> (App, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((MettleCorePlugin::new(Update), AttributePlugin));
    add_all(&mut app);
    let attributes = app.world().resource::<AttributeRegistry>().clone();
    let actors = (0..ACTORS)
        .map(|i| {
            app.world_mut()
                .spawn((bases(), modifiers(&attributes, f32::from(i))))
                .id()
        })
        .collect();
    app.update();
    app.update();
    (app, actors)
}

fn bench_attributes(c: &mut Criterion) {
    let mut group = c.benchmark_group("attributes");
    group.measurement_time(Duration::from_secs(4));
    // Every actor's modifiers changed: collect, fold, refresh, publish.
    let (mut app, actors) = attribute_app();
    group.bench_function("frame_all_changed_1000x12", |b| {
        b.iter(|| {
            for &actor in &actors {
                app.world_mut()
                    .get_mut::<Modifiers>(actor)
                    .unwrap()
                    .set_changed();
            }
            app.update();
        });
    });
    // Nothing changed: the frame's fixed cost.
    group.bench_function("frame_idle_1000x12", |b| b.iter(|| app.update()));
    // Reads in context: evaluated, not cached.
    let fire = AttributeContext::new().tag(FIRE);
    let plain = AttributeContext::new();
    let a3 = app.world().attribute_id::<A3>().unwrap();
    group.bench_function("value_at_in_context_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            let mut sum = 0.0;
            for &actor in &actors {
                sum += world.value_at(actor, a3, &fire).unwrap();
            }
            black_box(sum)
        });
    });
    group.bench_function("value_at_cached_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            let mut sum = 0.0;
            for &actor in &actors {
                sum += world.value_at(actor, a3, &plain).unwrap();
            }
            black_box(sum)
        });
    });
    group.finish();
}

/// A condition context over two tag counts.
struct Owned<'a>(&'a TagCounts, &'a Tags);
impl Context for Owned<'_> {
    fn tag(&self, tag: TagKey) -> bool {
        self.1.contains(tag)
    }

    fn has(&self, tag: TagKey) -> bool {
        self.0.contains(tag)
    }

    fn target(&self, tag: TagKey) -> bool {
        self.0.contains(tag)
    }

    fn source(&self, tag: TagKey) -> bool {
        self.0.contains(tag)
    }

    fn fact(&self, _: TagKey) -> Option<f32> {
        None
    }
}

fn bench_tags(c: &mut Criterion) {
    let mut group = c.benchmark_group("tags");
    let given: Vec<TagKey> = [
        "state.burning",
        "state.wet",
        "effect.affliction.burning",
        "cooldown.fireball",
        "damage.elemental.fire",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect();
    let mut counts = TagCounts::default();
    for tag in &given {
        counts.add(*tag);
    }
    let queries: Vec<TagKey> = [
        "state.stunned",
        "state",
        "effect.affliction",
        "cooldown.fireball",
        "damage.physical",
        "state.casting",
        "damage.elemental",
        "effect",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect();
    group.bench_function("contains_x8", |b| {
        b.iter(|| queries.iter().filter(|q| counts.contains(**q)).count());
    });
    group.bench_function("add_remove_depth3", |b| {
        let tag = TagKey::named("effect.affliction.burning");
        b.iter_batched_ref(
            || counts.clone(),
            |counts| {
                counts.add(tag);
                counts.remove(tag)
            },
            BatchSize::SmallInput,
        );
    });
    let own: Tags = [TagKey::named("damage.elemental.fire")]
        .into_iter()
        .collect();
    let condition = Condition::any([
        Condition::all([
            Condition::owner_has("state.burning"),
            !Condition::owner_has("state.wet"),
            Condition::read_tagged("damage.elemental"),
        ]),
        Condition::owner_has("state.stunned"),
    ]);
    group.bench_function("condition_holds", |b| {
        b.iter(|| condition.holds(&Owned(&counts, &own)));
    });
    group.finish();
}

/// An app with effects: hits routed through shields, and burning ticking.
fn effect_app() -> (App, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((MettleCorePlugin::new(Update), AttributePlugin, EffectPlugin))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
            16,
        )));
    add_all(&mut app);
    app.add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Shield>();
    app.add_route(IncomingDamage)
        .add_execution(Strike)
        .add_effect::<Hit>()
        .add_effect::<Routed>()
        .add_effect::<Struck>()
        .add_effect::<Burning>()
        .add_execution(Scorch)
        .add_effect::<Scorching>()
        .add_effect::<Aura>();
    let effects = vec![
        EffectData::instant("hit").modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0)),
        EffectData::instant("routed").modifier(EffectModifier::route::<IncomingDamage>(2.0)),
        EffectData::instant("struck").execution("strike"),
        EffectData::lasting("burning", 1_000_000.0)
            .tag("effect.affliction.burning")
            .period(0.016, false)
            .grant("state.burning")
            .modifier(EffectModifier::new::<Life>(Operation::Flat, 0.0)),
        EffectData::lasting("scorching", 1_000_000.0)
            .period(0.016, false)
            .execution("scorch"),
        // A tenth of the source's `A0`, a third of it in fire, followed live.
        EffectData::infinite("aura")
            .modifier(EffectModifier::new::<A2>(
                Operation::Flat,
                Magnitude::source::<A0>().live().times(0.1),
            ))
            .modifier(EffectModifier::new::<A4>(
                Operation::Flat,
                Magnitude::source::<A3>()
                    .live()
                    .tagged(FIRE)
                    .times(1.0 / 3.0),
            )),
    ];
    let world = app.world();
    let registry = world
        .resource::<EffectRegistry>()
        .recheck(
            &effects,
            EffectChecks {
                tags: &tags(),
                curves: &CurveRegistry::default(),
                attributes: world.resource(),
                executions: world.resource(),
                calculations: world.resource(),
                routes: world.resource(),
            },
        )
        .unwrap();
    app.insert_resource(registry);
    let attributes = app.world().resource::<AttributeRegistry>().clone();
    let actors: Vec<Entity> = (0..ACTORS)
        .map(|i| {
            app.world_mut()
                .spawn((
                    bases(),
                    (
                        Base::<Life>::new(1e9),
                        Base::<MaxLife>::new(1e9),
                        Base::<Shield>::new(1e9),
                    ),
                    modifiers(&attributes, f32::from(i)),
                ))
                .id()
        })
        .collect();
    app.update();
    app.update();
    (app, actors)
}

fn bench_effects(c: &mut Criterion) {
    let mut group = c.benchmark_group("effects");
    group.measurement_time(Duration::from_secs(4));
    let (mut app, actors) = effect_app();
    for (name, effect) in [
        ("apply_instant_x1000", Hit.spec()),
        ("apply_routed_x1000", Routed.spec()),
        ("apply_execution_x1000", Struck.spec()),
    ] {
        group.bench_function(name, |b| {
            b.iter(|| {
                for &actor in &actors {
                    app.world_mut().apply_effect(actor, effect.clone());
                }
            });
        });
    }
    // Every actor burning: each update, a period comes round on each.
    let (mut app, actors) = effect_app();
    for &actor in &actors {
        app.world_mut().apply_effect(actor, Burning);
    }
    app.update();
    group.bench_function("tick_1000_periodic", |b| b.iter(|| app.update()));
    group.finish();
}

fn bench_captures(c: &mut Criterion) {
    let mut group = c.benchmark_group("captures");
    group.measurement_time(Duration::from_secs(4));
    // Every actor scorched by the next: each update, a period comes round on
    // each, its execution reading a snapshot and a live capture.
    let (mut app, actors) = effect_app();
    for (i, &actor) in actors.iter().enumerate() {
        let source = actors[(i + 1) % actors.len()];
        app.world_mut()
            .apply_effect(actor, Scorching.spec().source(source));
    }
    app.update();
    group.bench_function("tick_1000_periodic_executions", |b| {
        b.iter(|| app.update());
    });
    // Every actor under one caster's aura, its `A0` changing each update:
    // each effect re-counted as it follows.
    let (mut app, actors) = effect_app();
    let caster = actors[0];
    for &actor in &actors[1..] {
        app.world_mut()
            .apply_effect(actor, Aura.spec().source(caster));
    }
    app.update();
    app.update();
    let mut power = 10.0;
    group.bench_function("follow_999_live_one_source", |b| {
        b.iter(|| {
            power += 1.0;
            app.world_mut()
                .get_mut::<Base<A0>>(caster)
                .unwrap()
                .set(power);
            app.update();
        });
    });
    // The same, nothing changing: what following costs idle.
    group.bench_function("follow_999_live_idle", |b| b.iter(|| app.update()));
    group.finish();
}

criterion_group!(
    benches,
    bench_attributes,
    bench_tags,
    bench_effects,
    bench_captures
);
criterion_main!(benches);
