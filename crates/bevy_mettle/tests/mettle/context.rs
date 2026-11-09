//! An effect's context: read by executions and calculations, carried down
//! chains, told with events and cues, and the game's own types on it.

use std::sync::{Arc, Mutex};

use bevy::ecs::entity::MapEntities;
use bevy::prelude::*;
use bevy_harness::TestApp;
use bevy_mettle::core::registry::{AbilityId, Name};
use bevy_mettle::prelude::*;
use serde::{Deserialize, Serialize};

use crate::common::{Life, actor, attributes, close, get, id};

/// The game's own: whether a shot was a headshot.
#[derive(
    ContextData,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    MapEntities
)]
struct Shot {
    headshot: bool,
}

/// The game's own: who a tracer marked.
#[derive(
    ContextData,
    Clone,
    Debug,
    PartialEq,
    Serialize,
    Deserialize,
    MapEntities
)]
struct Tracer {
    #[entities]
    marked: Entity,
}

/// A strike, running [`ShotDamage`], playing `cue.hit`.
#[derive(Effect)]
struct Strike;

/// A graze, its damage worked out by [`Falloff`].
#[derive(Effect)]
struct Graze;

/// A fuse, chaining the echo every way it can.
#[derive(Effect)]
struct Fuse;

/// 10 damage, twice that for a headshot; what it saw, noted.
#[derive(Execution)]
struct ShotDamage;

/// Notes what it saw.
#[derive(Execution)]
struct Witness;

/// As much damage as the hit is far from where it came from.
#[derive(Calculation)]
struct Falloff;

/// What executions saw, by effect, as they ran.
#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<(&'static str, EffectContext)>>>);

impl Seen {
    fn note(&self, by: &'static str, context: &ExecutionContext) {
        self.0.lock().unwrap().push((by, context.context().clone()));
    }

    /// What was seen since last asked.
    fn take(&self) -> Vec<(&'static str, EffectContext)> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

const TAGS: &[&str] = &["cue", "cue.hit"];

fn effects() -> Vec<EffectData> {
    vec![
        EffectData::instant("strike")
            .execution("shot_damage")
            .cue("cue.hit"),
        EffectData::instant("graze").modifier(EffectModifier::new::<Life>(
            Operation::Flat,
            Magnitude::Custom(Name::from_static("falloff")),
        )),
        EffectData::lasting("fuse", 0.5)
            .on_apply("echo")
            .on_remove("echo")
            .on_expire("echo")
            .stacking(Stacking {
                by: StackBy::Target,
                limit: 1,
                reapply: Reapply::Refresh,
                reset_period: false,
                expire: Expire::ClearAll,
                overflow: vec![Name::from_static("echo")],
            }),
        EffectData::instant("echo").execution("witness"),
    ]
}

/// An app with the effects, the game's types, and what executions see noted
/// in `seen`; then what `code` adds.
fn app(seen: &Seen, code: impl FnOnce(&mut App)) -> TestApp {
    let (damage, witness) = (seen.clone(), seen.clone());
    TestApp::builder()
        .with(attributes)
        .plugins(EffectPlugin::default())
        .with(move |app| {
            app.insert_resource(
                TagRegistry::build(TAGS.iter().map(|&name| TagKey::named(name))).unwrap(),
            )
            .init_resource::<CurveRegistry>()
            .add_context_data::<Shot>()
            .add_context_data::<Tracer>()
            .add_effect::<Strike>()
            .add_effect::<Graze>()
            .add_effect::<Fuse>()
            .add_execution::<ShotDamage>(move |cx: &ExecutionContext| {
                damage.note("strike", cx);
                let headshot = cx.get::<Shot>().is_some_and(|shot| shot.headshot);
                let damage = if headshot { 20.0 } else { 10.0 };
                vec![BaseChange::new(id::<Life>(), Operation::Flat, -damage)]
            })
            .add_execution::<Witness>(move |cx: &ExecutionContext| {
                witness.note("echo", cx);
                Vec::new()
            })
            .add_calculation::<Falloff>(|cx: &ExecutionContext| {
                match (cx.context().hit, cx.context().origin) {
                    (Some(hit), Some(origin)) => -hit.point.distance(origin),
                    _ => 0.0,
                }
            });
            code(app);
            let world = app.world();
            let registry = world
                .resource::<EffectRegistry>()
                .recheck(
                    &effects(),
                    EffectChecks {
                        tags: world.resource(),
                        curves: world.resource(),
                        attributes: world.resource(),
                        executions: world.resource(),
                        calculations: world.resource(),
                        routes: world.resource(),
                    },
                )
                .unwrap();
            app.insert_resource(registry);
        })
        .watch_triggered::<EffectExecuted>()
        .watch_triggered::<CueExecuted>()
        .build()
}

fn apply(app: &mut TestApp, target: Entity, spec: EffectSpec) {
    app.world_mut().apply_effect(target, spec);
    app.tick();
}

/// A spec of `effect` carrying everything: from `source`, struck by
/// `causer`, come from `object`, the ability, the hit, the origin, a
/// headshot, and the tracer marking `marked`.
fn everything(
    effect: impl Into<EffectSpec>,
    source: Entity,
    causer: Entity,
    object: Entity,
    marked: Entity,
) -> EffectSpec {
    effect
        .into()
        .source(source)
        .causer(causer)
        .object(object)
        .ability(ability())
        .hit(hit())
        .origin(Vec3::new(0.0, 0.0, 4.0))
        .with(Shot { headshot: true })
        .with(Tracer { marked })
}

fn ability() -> AbilityRef {
    AbilityRef {
        id: AbilityId::new(3),
        level: 2.0,
    }
}

fn hit() -> Hit {
    Hit {
        point: Vec3::new(3.0, 0.0, 0.0),
        normal: Vec3::X,
    }
}

#[test]
fn an_execution_reads_the_context_it_was_applied_with() {
    let seen = Seen::default();
    let mut app = app(&seen, |_| {});
    let npc = actor(&mut app, []);
    let archer = app.spawn(());
    let arrow = app.spawn(());
    let bow = app.spawn(());
    apply(&mut app, npc, Strike.spec().with(Shot { headshot: false }));
    assert!(close(get::<Life>(&app, npc), 40.0), "a body shot: 10");
    apply(&mut app, npc, everything(Strike, archer, arrow, bow, npc));
    assert!(close(get::<Life>(&app, npc), 20.0), "a headshot: 20");
    let seen = seen.take();
    let [(_, body), (_, head)] = &seen[..] else {
        panic!("two strikes seen: {seen:?}");
    };
    assert_eq!(body.get::<Shot>(), Some(&Shot { headshot: false }));
    assert_eq!(body.causer, None);
    assert_eq!(head.source, Some(archer));
    assert_eq!(head.causer, Some(arrow));
    assert_eq!(head.object, Some(bow));
    assert_eq!(head.ability, Some(ability()));
    assert_eq!(head.hit, Some(hit()));
    assert_eq!(head.origin, Some(Vec3::new(0.0, 0.0, 4.0)));
    assert_eq!(head.get::<Tracer>(), Some(&Tracer { marked: npc }));
}

#[test]
fn an_execution_reads_each_part_of_the_context() {
    let read = Arc::new(Mutex::new(None));
    let noted = read.clone();
    // In place of the strike's: what each part reads.
    let mut app = app(&Seen::default(), |app| {
        app.add_execution::<ShotDamage>(move |cx: &ExecutionContext| {
            *noted.lock().unwrap() = Some((
                cx.source(),
                cx.context().causer,
                cx.context().object,
                cx.context().ability,
                cx.context().hit,
                cx.context().origin,
                cx.get::<Shot>().cloned(),
                cx.get::<Tracer>().cloned(),
            ));
            Vec::new()
        });
    });
    let npc = actor(&mut app, []);
    let archer = app.spawn(());
    let arrow = app.spawn(());
    let bow = app.spawn(());
    apply(&mut app, npc, everything(Strike, archer, arrow, bow, npc));
    assert_eq!(
        read.lock().unwrap().take(),
        Some((
            Some(archer),
            Some(arrow),
            Some(bow),
            Some(ability()),
            Some(hit()),
            Some(Vec3::new(0.0, 0.0, 4.0)),
            Some(Shot { headshot: true }),
            Some(Tracer { marked: npc }),
        ))
    );
}

#[test]
fn a_calculation_reads_the_hit_and_the_origin() {
    let mut app = app(&Seen::default(), |_| {});
    let npc = actor(&mut app, []);
    // The hit 3 along, from 4 up: 5 apart.
    apply(
        &mut app,
        npc,
        Graze.spec().hit(hit()).origin(Vec3::new(0.0, 0.0, 4.0)),
    );
    assert!(
        close(get::<Life>(&app, npc), 45.0),
        "{}",
        get::<Life>(&app, npc)
    );
    apply(&mut app, npc, Graze.spec());
    assert!(close(get::<Life>(&app, npc), 45.0), "neither: none");
}

#[test]
fn an_effect_chained_carries_the_context_of_what_chained_it() {
    let seen = Seen::default();
    let mut app = app(&seen, |_| {});
    let npc = actor(&mut app, []);
    let archer = app.spawn(());
    let arrow = app.spawn(());
    let bow = app.spawn(());
    let first = everything(Fuse, archer, arrow, bow, npc);
    apply(&mut app, npc, first.clone());
    assert_eq!(seen.take(), [("echo", first.context.clone())], "on apply");
    // Applied again at the limit: its overflow carries the new one's.
    let second = Fuse.spec().causer(bow).with(Shot { headshot: false });
    apply(&mut app, npc, second.clone());
    assert_eq!(seen.take(), [("echo", second.context)], "overflowed");
    app.secs(0.5);
    // Run out: the first's context still, as removed and as expired.
    assert_eq!(
        seen.take(),
        [("echo", first.context.clone()), ("echo", first.context)],
        "removed, then expired"
    );
}

#[test]
fn events_and_cues_are_told_the_context() {
    let mut app = app(&Seen::default(), |_| {});
    let npc = actor(&mut app, []);
    let archer = app.spawn(());
    let arrow = app.spawn(());
    let bow = app.spawn(());
    let spec = everything(Strike, archer, arrow, bow, npc);
    apply(&mut app, npc, spec.clone());
    let executed = app.read_triggered::<EffectExecuted>();
    let [executed] = &executed[..] else {
        panic!("the strike, once: {executed:?}");
    };
    assert_eq!(executed.context, spec.context);
    assert_eq!(executed.context.causer, Some(arrow));
    assert_eq!(executed.context.hit, Some(hit()));
    assert_eq!(
        executed.context.get::<Shot>(),
        Some(&Shot { headshot: true })
    );
    let played = app.read_triggered::<CueExecuted>();
    let [played] = &played[..] else {
        panic!("the hit, once: {played:?}");
    };
    assert_eq!(played.tag.name(), "cue.hit");
    assert_eq!(played.context, spec.context);
    assert_eq!(played.context.causer, Some(arrow));
    assert_eq!(played.context.hit, Some(hit()));
    assert_eq!(
        played.context.get::<Tracer>(),
        Some(&Tracer { marked: npc })
    );
}

#[test]
fn a_type_added_again_replaces_its_own_and_no_other() {
    let marked = Entity::from_raw_u32(9).unwrap();
    let spec = Strike
        .spec()
        .with(Shot { headshot: false })
        .with(Tracer { marked })
        .with(Shot { headshot: true });
    assert_eq!(
        spec.context.get::<Shot>(),
        Some(&Shot { headshot: true }),
        "the last"
    );
    assert_eq!(spec.context.get::<Tracer>(), Some(&Tracer { marked }));
    assert_eq!(
        spec,
        Strike
            .spec()
            .with(Shot { headshot: true })
            .with(Tracer { marked }),
        "the same, as replaced in place"
    );
    assert_ne!(
        Strike.spec().with(Shot { headshot: true }),
        Strike.spec().with(Shot { headshot: false }),
        "its game data counts"
    );
    assert_ne!(Strike.spec().with(Shot { headshot: true }), Strike.spec());
    let mut given = Strike.spec();
    given.context = EffectContext::default().with(Tracer { marked });
    assert_eq!(
        given,
        Strike.spec().with(Tracer { marked }),
        "a context given whole"
    );
    assert_eq!(
        Strike
            .spec()
            .with(Shot { headshot: true })
            .with(Tracer { marked }),
        Strike
            .spec()
            .with(Tracer { marked })
            .with(Shot { headshot: true }),
        "one of each type, whatever order they were added in"
    );
    assert_ne!(
        Strike.spec().with(Shot { headshot: true }),
        Strike.spec().with(Tracer { marked }),
        "the same count of others isn't the same"
    );
}
