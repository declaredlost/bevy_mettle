//! Mettle's hot paths, measured: attributes kept and read, tags and
//! conditions checked, effects applied, routed, stacked, served and ticked,
//! captures executed, snapshotted and followed, and abilities activated, for
//! 1000 actors; and a mixed frame of 300.
//!
//! `cargo bench -p bevy_mettle --bench mettle`.

use std::hint::black_box;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_mettle::core::condition::Context;
use bevy_mettle::effect::cue::CueBursts;
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
#[route(target(Shield, live))]
struct IncomingDamage;

impl RouteFn for IncomingDamage {
    fn route(&self, effect: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
        let shield = effect.captured::<Shield>(Side::Target).unwrap_or_default();
        let absorbed = damage.min(shield);
        vec![
            BaseChange::of::<Shield>(Operation::Flat, -absorbed),
            BaseChange::of::<Life>(Operation::Flat, absorbed - damage),
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
        vec![BaseChange::of::<Life>(Operation::Flat, -effect.level())]
    }
}

/// What a scorch executes: its source's `A0` as it applied, less its
/// target's `A1` as it is now, off its life.
#[derive(Execution)]
#[execution(source(A0), target(A1, live))]
struct Scorch;

impl ExecutionFn for Scorch {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let power = effect.captured::<A0>(Side::Source).unwrap_or_default();
        let resist = effect.captured::<A1>(Side::Target).unwrap_or_default();
        vec![BaseChange::of::<Life>(
            Operation::Flat,
            (resist - power).min(0.0),
        )]
    }
}

/// Effects capturing what they read, as code names them.
#[derive(Effect)]
struct Scorching;
#[derive(Effect)]
struct Aura;
/// Its source's `A0` bonus and fire `A3` as the spec was made, off life.
#[derive(Effect)]
struct Snapped;

/// Effects stacking, served and ticking, as code names them.
#[derive(Effect)]
struct Stacked;
#[derive(Effect)]
struct Ward;
#[derive(Effect)]
struct Flare;
#[derive(Effect)]
struct Dot;
/// What an ability costs, and its cooldown.
#[derive(Effect)]
#[effect(name = "bolt.cost")]
struct BoltCost;
#[derive(Effect)]
#[effect(name = "bolt.cooldown")]
struct BoltCooldown;

const ACTORS: u16 = 1000;
const FIRE: &str = "damage.elemental.fire";
/// What the scenario frame has: actors, casters among them, hits each update.
const SCENARIO_ACTORS: u16 = 300;
const CASTERS: usize = 10;
const HITS: usize = 20;

/// Each actor's own modifiers: 16, a quarter of them scoped to fire reads.
fn modifiers(attributes: &AttributeRegistry, seed: f32) -> Modifiers {
    own_modifiers(attributes, seed, |modifier| modifier.read_tagged(FIRE))
}

/// The same 16, none of them conditioned: no context changes what they fold
/// to.
fn unconditioned(attributes: &AttributeRegistry, seed: f32) -> Modifiers {
    own_modifiers(attributes, seed, |modifier| modifier)
}

/// Each actor's own modifiers, the last four on `A3` as `scope` makes them.
fn own_modifiers(
    attributes: &AttributeRegistry,
    seed: f32,
    scope: impl Fn(ModifierSpec) -> ModifierSpec,
) -> Modifiers {
    let mut all = vec![
        ModifierSpec::flat::<A0>(seed),
        ModifierSpec::flat::<A1>(1.0),
        ModifierSpec::flat::<A2>(2.0),
        ModifierSpec::flat::<A3>(3.0),
        ModifierSpec::flat::<A4>(4.0),
        ModifierSpec::flat::<A5>(5.0),
        ModifierSpec::flat::<A6>(6.0),
        ModifierSpec::flat::<A7>(7.0),
        ModifierSpec::flat::<A8>(8.0),
        ModifierSpec::flat::<A9>(9.0),
        ModifierSpec::flat::<A10>(10.0),
        ModifierSpec::flat::<A11>(11.0),
    ];
    for _ in 0..4 {
        all.push(scope(ModifierSpec::flat::<A3>(0.5)));
    }
    all.iter()
        .map(|spec| {
            spec.resolve(attributes)
                .expect("an attribute the benches add")
        })
        .collect()
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
        "cue",
        "cue.hit",
    ]
    .into_iter()
    .map(TagKey::named)
    .collect();
    TagRegistry::build(defs).unwrap()
}

/// An app keeping attributes every update, with `ACTORS` actors.
fn attribute_app() -> (App, Vec<Entity>) {
    attribute_app_with(modifiers)
}

/// The same, each actor's own modifiers made by `own`.
fn attribute_app_with(own: fn(&AttributeRegistry, f32) -> Modifiers) -> (App, Vec<Entity>) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins((MettleCorePlugin::new(Update), AttributePlugin));
    add_all(&mut app);
    let attributes = app.world().resource::<AttributeRegistry>().clone();
    let actors = (0..ACTORS)
        .map(|i| {
            app.world_mut()
                .spawn((bases(), own(&attributes, f32::from(i))))
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
    group.bench_function("value_in_context_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            let mut sum = 0.0;
            for &actor in &actors {
                sum += world.value::<A3>(actor, &fire).unwrap();
            }
            black_box(sum)
        });
    });
    group.bench_function("value_cached_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            let mut sum = 0.0;
            for &actor in &actors {
                sum += world.value::<A3>(actor, &plain).unwrap();
            }
            black_box(sum)
        });
    });
    // One base of the 12 changed on every actor: what re-folding costs when
    // only part of an actor moved.
    let mut base = 10.0;
    group.bench_function("frame_one_base_changed_1000x12", |b| {
        b.iter(|| {
            base += 1.0;
            for &actor in &actors {
                app.world_mut()
                    .get_mut::<Base<A5>>(actor)
                    .unwrap()
                    .set(base);
            }
            app.update();
        });
    });
    // Reads in a context nothing depends on: no modifier is conditioned, so
    // the context can't change what they fold to.
    let (app, actors) = attribute_app_with(unconditioned);
    group.bench_function("value_in_context_unconditioned_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            let mut sum = 0.0;
            for &actor in &actors {
                sum += world.value::<A3>(actor, &fire).unwrap();
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

/// The effects the benches apply, as content defines them.
fn effects() -> Vec<EffectData> {
    vec![
        EffectData::instant(Key::of::<Hit>())
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0)),
        EffectData::instant(Key::of::<Routed>())
            .modifier(EffectModifier::route::<IncomingDamage>(2.0)),
        EffectData::instant(Key::of::<Struck>()).execution(Key::of::<Strike>()),
        EffectData::lasting(Key::of::<Burning>(), 1_000_000.0)
            .tag("effect.affliction.burning")
            .period(0.016, false)
            .owner_tag("state.burning")
            .modifier(EffectModifier::new::<Life>(Operation::Flat, 0.0)),
        EffectData::lasting(Key::of::<Scorching>(), 1_000_000.0)
            .period(0.016, false)
            .execution(Key::of::<Scorch>()),
        // A tenth of the source's `A0`, a third of it in fire, followed live.
        EffectData::infinite(Key::of::<Aura>())
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
        // Its source's `A0` bonus and fire `A3`, as the spec was made.
        EffectData::instant(Key::of::<Snapped>())
            .modifier(EffectModifier::new::<Life>(
                Operation::Flat,
                Magnitude::source::<A0>().bonus().times(-0.01),
            ))
            .modifier(EffectModifier::new::<Life>(
                Operation::Flat,
                Magnitude::source::<A3>().tagged(FIRE).times(-0.01),
            )),
        // Up to 5 stacks on a target, each refreshing its time.
        EffectData::lasting(Key::of::<Stacked>(), 1_000_000.0)
            .tag("effect.affliction")
            .stacking(Stacking {
                by: StackBy::Target,
                limit: 5,
                reapply: Reapply::Refresh,
                reset_period: false,
                expire: Expire::ClearAll,
                overflow: Vec::new(),
            })
            .modifier(EffectModifier::new::<A5>(Operation::Flat, 1.0)),
        EffectData::lasting(Key::of::<Ward>(), 1_000_000.0)
            .modifier(EffectModifier::new::<A6>(Operation::Flat, 1.0)),
        EffectData::instant(Key::of::<Flare>())
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0))
            .cue("cue.hit"),
        // Damage over time: every half second.
        EffectData::lasting(Key::of::<Dot>(), 1_000_000.0)
            .period(0.5, false)
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0)),
        // What an ability costs, and its cooldown: shorter than an update, run
        // out by the next.
        EffectData::instant(Key::of::<BoltCost>())
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -1.0)),
        EffectData::lasting(Key::of::<BoltCooldown>(), 0.01).owner_tag("cooldown.fireball"),
    ]
}

/// The stacks of `actor`'s first active effect.
fn stacks(app: &App, actor: Entity) -> u32 {
    let effect = app
        .world()
        .get::<ActiveEffects>(actor)
        .unwrap()
        .iter()
        .next();
    app.world()
        .get::<ActiveEffect>(effect.unwrap())
        .unwrap()
        .stacks()
}

/// How many of `actor`'s active effects are kept for clients.
fn replicas(app: &App, actor: Entity) -> usize {
    app.world()
        .get::<ActiveEffects>(actor)
        .map_or(0, |effects| {
            effects
                .iter()
                .filter(|&effect| app.world().get::<EffectReplica>(effect).is_some())
                .count()
        })
}

/// An app with effects: hits routed through shields, and burning ticking.
fn effect_app() -> (App, Vec<Entity>) {
    effect_app_with(ACTORS, |_| {})
}

/// An app with effects, as a server keeping what its clients are sent.
fn server_app() -> (App, Vec<Entity>) {
    effect_app_with(ACTORS, |app| {
        app.insert_resource(NetRole::Server).insert_resource(tags());
    })
}

/// An app with effects and `actors` actors, `extra` setting it up further
/// once its effects are checked, before its actors are spawned.
fn effect_app_with(actors: u16, extra: impl FnOnce(&mut App)) -> (App, Vec<Entity>) {
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
        .add_effect::<Aura>()
        .add_effect::<Snapped>()
        .add_effect::<Stacked>()
        .add_effect::<Ward>()
        .add_effect::<Flare>()
        .add_effect::<Dot>()
        .add_effect::<BoltCost>()
        .add_effect::<BoltCooldown>();
    let effects = effects();
    let world = app.world();
    let registry = world
        .resource::<EffectRegistry>()
        .rebuild(
            &effects,
            EffectScope {
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
    extra(&mut app);
    let attributes = app.world().resource::<AttributeRegistry>().clone();
    let actors: Vec<Entity> = (0..actors)
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
    // Each actor's stacks: applied ten times a round, five stacking up and
    // five at the limit, refreshing; taken off between rounds, untimed.
    let (mut app, actors) = effect_app();
    let stacked: Tags = [TagKey::named("effect.affliction")].into_iter().collect();
    for _ in 0..6 {
        app.world_mut().apply_effect(actors[0], Stacked);
    }
    assert_eq!(stacks(&app, actors[0]), 5, "stacked up to the limit");
    app.world_mut().remove_effects(actors[0], &stacked);
    app.update();
    assert!(
        app.world()
            .get::<ActiveEffects>(actors[0])
            .is_none_or(|effects| effects.iter().next().is_none()),
        "taken off between rounds"
    );
    group.bench_function("stack_refresh_x1000", |b| {
        b.iter_custom(|iters| {
            let mut spent = Duration::ZERO;
            for round in 0..iters {
                if round % 10 == 0 {
                    for &actor in &actors {
                        app.world_mut().remove_effects(actor, &stacked);
                    }
                    app.update();
                }
                let start = Instant::now();
                for &actor in &actors {
                    app.world_mut().apply_effect(actor, Stacked);
                }
                spent += start.elapsed();
            }
            spent
        });
    });
    // A server, each actor with a lasting effect: each update, its replicas
    // kept in step for its clients.
    let (mut app, actors) = server_app();
    for &actor in &actors {
        app.world_mut().apply_effect(actor, Ward);
    }
    app.update();
    assert!(
        actors.iter().all(|&actor| replicas(&app, actor) == 1),
        "each effect kept for clients"
    );
    group.bench_function("server_sync_1000_lasting", |b| b.iter(|| app.update()));
    // A server, an instant effect with a cue on each actor: each burst kept
    // for its clients. An update between, untimed, lets go of the old.
    let (mut app, actors) = server_app();
    app.world_mut().apply_effect(actors[0], Flare);
    assert_eq!(
        app.world()
            .get::<CueBursts>(actors[0])
            .map(|b| b.recent.len()),
        Some(1),
        "the cue kept for clients"
    );
    group.bench_function("server_cue_bursts_x1000", |b| {
        b.iter_custom(|iters| {
            let mut spent = Duration::ZERO;
            for _ in 0..iters {
                let start = Instant::now();
                for &actor in &actors {
                    app.world_mut().apply_effect(actor, Flare);
                }
                spent += start.elapsed();
                app.update();
            }
            spent
        });
    });
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
    // A spec's snapshot taken from each actor: a bonus and a tagged read.
    let (app, actors) = effect_app();
    assert_eq!(
        app.world()
            .snapshot(Snapped.source(actors[0]))
            .snapshot
            .len(),
        2,
        "both captures taken"
    );
    group.bench_function("snapshot_take_x1000", |b| {
        b.iter(|| {
            let world = app.world();
            for &actor in &actors {
                black_box(world.snapshot(Snapped.source(actor)));
            }
        });
    });
    group.finish();
}

/// What runs the bench's ability: nothing, it's ended at once.
#[cfg(feature = "ability")]
#[derive(Component, Default, Behaviour)]
struct Cast;

/// An ability costing a life and cooling down for 10 ms.
#[cfg(feature = "ability")]
#[derive(Ability)]
struct Bolt;

/// An app playing as `role`, each actor granted a bolt.
#[cfg(feature = "ability")]
fn ability_app(role: NetRole) -> (App, Vec<Entity>) {
    let (mut app, actors) = effect_app_with(ACTORS, move |app| {
        app.add_plugins(AbilityPlugin)
            .insert_resource(role)
            .insert_resource(tags())
            .add_behaviour::<Cast>()
            .add_ability::<Bolt>();
        let world = app.world_mut();
        let abilities = world
            .resource::<AbilityRegistry>()
            .rebuild(
                &[AbilityData::new(Key::of::<Bolt>(), Key::of::<Cast>())
                    .cost(Key::of::<BoltCost>())
                    .cooldown(Key::of::<BoltCooldown>())],
                &[],
                AbilityScope {
                    tags: world.resource(),
                    effects: world.resource(),
                    behaviours: world.resource(),
                },
            )
            .unwrap();
        world.insert_resource(abilities);
    });
    for &actor in &actors {
        app.world_mut().grant_ability(actor, Bolt).expect("granted");
    }
    app.update();
    (app, actors)
}

/// Each actor's bolt activated (paid, cooled down) and ended, then an update
/// running the cooldowns out.
#[cfg(feature = "ability")]
fn activate_end(app: &mut App, actors: &[Entity]) {
    for &actor in actors {
        let ability = app
            .world_mut()
            .activate_ability(actor, Bolt)
            .expect("off cooldown, and affordable")
            .ability();
        app.world_mut().end_ability(ability);
    }
    app.update();
}

/// Each request a client sent answered as a server would, accepted: what
/// it confirms next update. Unanswered, its predictions would pile up.
#[cfg(feature = "ability")]
fn answer(app: &mut App) {
    let world = app.world_mut();
    let requests: Vec<AbilityRequest> = world
        .resource_mut::<Messages<AbilityRequest>>()
        .drain()
        .collect();
    for request in requests {
        world.write_message(AbilityReply {
            actor: request.actor,
            ability: request.ability,
            activation: request.activation,
            rejected: None,
        });
    }
}

#[cfg(feature = "ability")]
fn bench_abilities(c: &mut Criterion) {
    let mut group = c.benchmark_group("abilities");
    group.measurement_time(Duration::from_secs(4));
    // Standalone: each actor's bolt activated, paid for, ended, and its
    // cooldown run out.
    let (mut app, actors) = ability_app(NetRole::Standalone);
    // Again and again: its cooldown run out each time.
    activate_end(&mut app, &actors);
    activate_end(&mut app, &actors);
    group.bench_function("activate_end_x1000", |b| {
        b.iter(|| activate_end(&mut app, &actors));
    });
    // A client: each predicted, asked of the server, and answered at once, as
    // a server on the same machine would.
    let (mut app, actors) = ability_app(NetRole::Client);
    let predict = |app: &mut App| {
        for &actor in &actors {
            let ability = app
                .world_mut()
                .activate_ability(actor, Bolt)
                .expect("off cooldown, and affordable")
                .ability();
            app.world_mut().end_ability(ability);
        }
        answer(app);
        app.update();
    };
    predict(&mut app);
    predict(&mut app);
    group.bench_function("predict_x1000", |b| b.iter(|| predict(&mut app)));
    group.finish();
}

#[cfg(not(feature = "ability"))]
fn bench_abilities(_: &mut Criterion) {}

/// A realistic frame: 300 actors, each burning down every half second and
/// under one of 10 casters' auras, 20 of them struck each update.
fn bench_scenario(c: &mut Criterion) {
    let mut group = c.benchmark_group("scenario");
    group.measurement_time(Duration::from_secs(4));
    let (mut app, actors) = effect_app_with(SCENARIO_ACTORS, |_| {});
    let casters = &actors[..CASTERS];
    for (i, &actor) in actors.iter().enumerate() {
        let caster = casters[(i + 1) % CASTERS];
        app.world_mut()
            .apply_effect(actor, Aura.spec().source(caster));
    }
    // Their periods spread over the half second: ten actors a frame.
    for chunk in actors.chunks(10) {
        for &actor in chunk {
            app.world_mut().apply_effect(actor, Dot);
        }
        app.update();
    }
    let mut seed = 0x2545_f491_u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as usize
    };
    group.bench_function("mixed_300_actors", |b| {
        b.iter(|| {
            for _ in 0..HITS {
                let target = actors[next() % actors.len()];
                let caster = casters[next() % CASTERS];
                app.world_mut()
                    .apply_effect(target, Struck.spec().source(caster));
            }
            app.update();
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_attributes,
    bench_tags,
    bench_effects,
    bench_captures,
    bench_abilities,
    bench_scenario
);
criterion_main!(benches);
