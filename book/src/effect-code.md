# Effects in code

An effect's keys say most of what it does. Some numbers they can't: fire
damage that grows with the caster's spell power and shrinks with the
target's fire resistance, or a burn that lasts longer the stronger its
caster. Those are code too, named by the effect. Mettle has three kinds:

- **Executions** are formulas run as an effect executes. They return the
  changes to make to the target's bases. Unreal calls them execution
  calculations.
- **Calculations** work out one number, a [`Magnitude`] of `Custom`. They can
  stand in for any magnitude: a modifier's value, a duration, a period.
- **Routes** take an amount no actor holds, such as incoming damage, and turn
  it into changes to attributes actors do hold: see [Routes](#routes).

What none of them can do, such as spawning something or playing a sound,
goes in an observer of the effect's events: see [What's
told](applying-effects.md#whats-told).

## Adding code

Derive [`Execution`] or [`Calculation`] on a struct, its name its type's in
`snake_case` unless `#[execution(name = "..")]` or
`#[calculation(name = "..")]` says otherwise. What it runs is a trait on the
same type: [`ExecutionFn::execute`] returns the changes,
[`CalculationFn::calculate`] the number, each with `&self`, so the struct's
fields are what it's set up with. Add a value of it with [`add_execution`]
and [`add_calculation`] from [`EffectApp`], as attributes and behaviours
are; it's numbered as it's added, in the [`ExecutionRegistry`] or the
[`CalculationRegistry`], and adding its type again replaces it. The derive
also declares the attributes it [captures](#captures), `source(..)` and
`target(..)`.

An effect names it by type: the key `execution::<FireDamage>` runs an
execution, and `Magnitude::calculation::<BurnTime>()` is a calculation's
magnitude. Checking the effects resolves each to its number, and rejects
one the app didn't add:

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use bevy_mettle::effect::code::ExecutionRegistry;
# use serde::{Deserialize, Serialize};
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Attribute)]
# #[attribute(max = MaxResistance)]
# struct Resistance;
# #[derive(Attribute)]
# #[attribute(base = 0.75)]
# struct MaxResistance;
# #[derive(Attribute)]
# #[attribute(base = 10.0)]
# struct SpellPower;
/// How hard a hit is, set by the code applying it.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Damage {
    amount: f32,
}

/// The spec's damage, more with the caster's spell power, less with the
/// target's fire resistance, scaled by the stacks of it.
#[derive(Execution)]
#[execution(source(SpellPower), target(Resistance, tagged("damage.elemental.fire")))]
struct FireDamage; // "fire_damage"

impl ExecutionFn for FireDamage {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let damage = effect.get::<Damage>().map_or(0.0, |damage| damage.amount);
        let power = effect.captured::<SpellPower>(Side::Source).unwrap_or_default();
        let resisted = effect.captured::<Resistance>(Side::Target).unwrap_or_default();
        let dealt = damage * (1.0 + power / 100.0) * (1.0 - resisted) * effect.stacks as f32;
        vec![BaseChange::of::<Life>(Operation::Flat, -dealt)]
    }
}

/// Two seconds, and one more for every `per` spell power.
#[derive(Calculation)]
#[calculation(source(SpellPower))]
struct BurnTime { per: f32 } // "burn_time"

impl CalculationFn for BurnTime {
    fn calculate(&self, effect: &ExecutionContext) -> f32 {
        let power = effect.captured::<SpellPower>(Side::Source).unwrap_or_default();
        2.0 + power / self.per
    }
}

fn code(app: &mut App) {
    app.add_execution(FireDamage)
        .add_calculation(BurnTime { per: 25.0 });
}

let mut app = App::new();
code(&mut app);
let executions = app.world().resource::<ExecutionRegistry>();
let id = executions.id_of::<FireDamage>().unwrap();
assert_eq!(executions.id("fire_damage"), Some(id));
assert_eq!(executions.name(id), Some("fire_damage"));
```

## The fireball hits

A caster with 25 spell power hits a mage with 50% fire resistance for 20:

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Attribute)]
# #[attribute(max = MaxResistance)]
# struct Resistance;
# #[derive(Attribute)]
# #[attribute(base = 0.75)]
# struct MaxResistance;
# #[derive(Attribute)]
# #[attribute(base = 10.0)]
# struct SpellPower;
# #[derive(Tag)] #[tag(name = "damage")] struct DamageTag;
# #[derive(Tag)] #[tag(parent = DamageTag)] struct Elemental;
# #[derive(Tag)] #[tag(parent = Elemental)] struct Fire;
# #[derive(Tag)] #[tag(name = "effect")] struct Effects;
# #[derive(Tag)] #[tag(parent = Effects)] struct Affliction;
# #[derive(Tag)] #[tag(parent = Affliction, name = "burning")] struct Burns;
# #[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
# #[reflect(ContextData)] struct Damage { amount: f32 }
# #[derive(Execution)]
# #[execution(source(SpellPower), target(Resistance, tagged("damage.elemental.fire")))]
# struct FireDamage;
# #[derive(Calculation)] #[calculation(source(SpellPower))] struct BurnTime { per: f32 }
# fn power(effect: &ExecutionContext) -> f32 {
#     effect.captured::<SpellPower>(Side::Source).unwrap_or_default()
# }
# impl ExecutionFn for FireDamage {
#     fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
#         let damage = effect.get::<Damage>().map_or(0.0, |damage| damage.amount);
#         let resisted = effect.captured::<Resistance>(Side::Target).unwrap_or_default();
#         let dealt = damage * (1.0 + power(effect) / 100.0) * (1.0 - resisted) * effect.stacks as f32;
#         vec![BaseChange::of::<Life>(Operation::Flat, -dealt)]
#     }
# }
# impl CalculationFn for BurnTime {
#     fn calculate(&self, effect: &ExecutionContext) -> f32 {
#         2.0 + power(effect) / self.per
#     }
# }
#[derive(Effect)]
#[effect(name = "fireball.hit", tag = Fire, execution::<FireDamage>)]
struct FireballHit;

#[derive(Effect)]
#[effect(lasting = Magnitude::calculation::<BurnTime>(), period = 0.5)]
#[effect(tag = Burns, execution::<FireDamage>)]
struct Burning;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .add_attribute::<Resistance>()
#     .add_attribute::<MaxResistance>()
#     .add_attribute::<SpellPower>()
#     .add_tag::<Fire>()
#     .add_tag::<Burns>()
#     .register_type::<Damage>()
#     .add_execution(FireDamage)
#     .add_calculation(BurnTime { per: 25.0 })
#     .add_effects::<(FireballHit, Burning)>()
#     .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
# app.finish();
# app.cleanup();
# app.update();
let caster = app.world_mut().spawn(Base::<SpellPower>::new(25.0)).id();
let mage = app
    .world_mut()
    .spawn((
        Base::<Life>::default(),
        Base::<MaxLife>::default(),
        Base::<Resistance>::new(0.5),
        Base::<MaxResistance>::default(),
    ))
    .id();
// One fixed step, for their attributes to be worked out.
app.update();

let hit = FireballHit.with(Damage { amount: 20.0 }).source(caster);
app.world_mut().apply_effect(mage, hit);
// 20, 25% more, half resisted.
assert_eq!(app.world().get::<Base<Life>>(mage).unwrap().get(), 87.5);

let burn = Burning.with(Damage { amount: 2.0 }).source(caster);
app.world_mut().apply_effect(mage, burn);
let effect = app.world().get::<ActiveEffects>(mage).unwrap().iter().next().unwrap();
let remaining = app.world().get::<ActiveEffect>(effect).unwrap().remaining();
assert_eq!(remaining, Some(3.0), "two seconds, and one for 25 spell power");
```

## What code sees

An execution or a calculation gets an [`ExecutionContext`]: the world, read
only, and what's being applied.

| | What it is |
|---|---|
| `world` | The world, read only: the raw escape hatch, for what a capture can't read |
| `target` | Who the effect is applied to |
| `def` | The effect's checked definition: its number, name and tags |
| `effect()` | The effect's number, an `EffectId` |
| `spec` | The [`EffectSpec`] applied |
| `stacks` | How many stacks of it there are: 1 unless it stacks |
| `level()` | The spec's level |
| `context()`, `source()` | What the spec carries, and its source: see [Context](applying-effects.md#context) |
| `get::<C>()` | What the applying code put on the context of type `C`, a [`ContextData`]: how hard it hits, what struck |
| `captured::<A>(from)` | What it declared it [captures](#captures) of the source's or the target's `A` |
| `owned(entity)` | What an entity owns: its [`OwnedTags`] |
| `breakdown::<A>(entity, read)` | An entity's attribute's modifiers, summed but not applied: read now, never snapshotted or predicted |

[Reading in context](reading.md) explains reads in context.

## Captures

What an execution, a calculation or a router reads of the source's and the
target's attributes, it declares on its derive, as Unreal's execution
declares its capture definitions:

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] struct SpellPower;
# #[derive(Attribute)] struct Armour;
#[derive(Execution)]
#[execution(source(SpellPower, bonus), target(Armour, live, tagged("damage.fire")))]
struct FireDamage;
# assert_eq!(FireDamage::captures(), vec![
#     Magnitude::source::<SpellPower>().bonus(),
#     Magnitude::target::<Armour>().live().tagged("damage.fire"),
# ]);
```

Each is an attribute's type, then the options of an
[`AttributeRead`](effects.md#captures): `base` or `bonus` for which of its
values, `live` to read it live, and `tagged(..)` for the tags it's read in.
An effect running the code captures them with its own magnitudes'
captures, as a snapshot or live. The code reads them with
`effect.captured::<A>(Side::Source)`, as Unreal's
`AttemptCalculateCapturedAttributeMagnitude` does: `None` for one it didn't
declare.

Declared, a capture is the effect's like any other. A snapshot of the
source's goes in the spec's [snapshot](applying-effects.md#snapshots), so a
fireball landing late hits with its caster's spell power as it was cast. It
goes with the effect's replica to clients. Its tags are checked with the
effect. Captures are how effect code reads attributes. `breakdown` reads
the world as it is, captured by nothing; so does `world`, for what no
capture can name, such as a third actor's.

A periodic burn that reads its target's resistance live deals more the
moment the resistance goes:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
#[derive(Attribute)]
struct Resistance;

/// 10 a second, less what the target resists as it ticks.
#[derive(Execution)]
#[execution(target(Resistance, live))]
struct Sear;

impl ExecutionFn for Sear {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let resisted = effect.captured::<Resistance>(Side::Target).unwrap_or_default();
        vec![BaseChange::of::<Life>(Operation::Flat, -10.0 * (1.0 - resisted))]
    }
}

#[derive(Effect)]
#[effect(lasting = 10.0, period = 1.0, execute_on_apply, execution::<Sear>)]
struct Searing;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   // One fixed step, 1/64 of a second, each update.
#   .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .add_attribute::<Life>()
    .add_attribute::<Resistance>()
    .add_execution(Sear)
    .add_effect::<Searing>();
app.update();
let mage = app
    .world_mut()
    .spawn((Base::<Life>::default(), Base::<Resistance>::new(0.5)))
    .id();
app.update();
let life = |app: &App| app.world().get::<Base<Life>>(mage).unwrap().get();

app.world_mut().apply_effect(mage, Searing);
assert_eq!(life(&app), 95.0, "half resisted");

// The resistance is stripped: the next tick reads it as it is.
app.world_mut().get_mut::<Base<Resistance>>(mage).unwrap().set(0.0);
for _ in 0..64 {
    app.update();
}
assert_eq!(life(&app), 85.0, "none resisted");
```

Without `live`, the target's resistance would be captured as the burn
applied, and every tick would deal 5.

## Executions

An execution runs whenever its effect executes: as an instant effect applies,
or as a periodic effect's period comes round. It runs after the effect's own
modifiers have been worked out, and returns a list of [`BaseChange`]s.
`BaseChange::of::<Life>(op, value)` changes the target's life's base as an
executing modifier would: `Flat` adds, `Additive` scales, `Override` sets.
The base is then rounded and kept within its limits.

A change is to the target's base, unless `.on_source()` says it's to the
source's: life stolen back. An execution can also route into a
[route](#routes) it declares, `#[execution(routes(IncomingDamage))]`, with
`BaseChange::route::<IncomingDamage>(damage)`, for its router to route.

A change named by type is numbered as it's applied. One to an attribute or a
route the app hasn't added is warned of once, and dropped; so is one into a
route the execution doesn't declare.

Executions don't scale with stacks by themselves, since they work out their
own numbers. Multiply by `stacks` where the effect should count once per
stack, as `FireDamage` does.

A breakdown lets an execution combine a caster's modifiers with a target's.
Damage increased by 30% on the caster, and damage taken increased by 20% on
the target, add up to 50% more, as they would if they were one attribute's:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct Damage;
# #[derive(Attribute)] #[attribute(base = 0.0)] struct DamageTaken;
# #[derive(Tag)] #[tag(name = "damage")] struct DamageTag;
# #[derive(Tag)] #[tag(parent = DamageTag)] struct Elemental;
# #[derive(Tag)] #[tag(parent = Elemental)] struct Fire;
# #[derive(Execution)] struct Strike;
impl ExecutionFn for Strike {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let read = AttributeContext::new().tag(Fire);
        let (Some(caster), Some(taken)) = (
            effect.source().and_then(|caster| effect.breakdown::<Damage>(caster, &read)),
            effect.breakdown::<DamageTaken>(effect.target, &read),
        ) else {
            return Vec::new();
        };
        let mut hit = caster;
        hit.additive += taken.additive;
        vec![BaseChange::of::<Life>(Operation::Flat, -hit.total())]
    }
}
# App::new().add_execution(Strike);
```

## Routes

Every hit takes the shield's share first, then life's. Written into each
execution, that's the same code many times over. A route puts it in one
place: hits route into `incoming_damage`, and its router decides what that
does to the attributes actors have.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
#[derive(Attribute)]
#[attribute(min = 0.0)]
struct Shield;

/// Damage dealt, before the shield takes its share: the target's shield
/// read as each hit's routed.
#[derive(Route)]
#[route(target(Shield, live))]
struct IncomingDamage;

#[derive(Effect)]
#[effect(route::<IncomingDamage>(40.0))]
struct Hit;

/// The shield takes what it can and life the rest; the attacker steals back a
/// fifth of what life lost.
impl RouteFn for IncomingDamage {
    fn route(&self, effect: &ExecutionContext, damage: f32) -> Vec<BaseChange> {
        let held = effect.captured::<Shield>(Side::Target).unwrap_or_default();
        let absorbed = damage.min(held);
        let taken = damage - absorbed;
        vec![
            BaseChange::of::<Shield>(Operation::Flat, -absorbed),
            BaseChange::of::<Life>(Operation::Flat, -taken),
            BaseChange::of::<Life>(Operation::Flat, taken / 5.0).on_source(),
        ]
    }
}

# use bevy::time::TimeUpdateStrategy;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   // One fixed step, where mettle runs, each update.
#   .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .add_attribute::<Life>()
    .add_attribute::<MaxLife>()
    .add_attribute::<Shield>()
    .add_route(IncomingDamage)
    .add_effect::<Hit>();
app.update();
let base = |app: &App, entity| app.world().get::<Base<Life>>(entity).unwrap().get();
let mage = app
    .world_mut()
    .spawn((Base::<Life>::default(), Base::<MaxLife>::default(), Base::<Shield>::new(30.0)))
    .id();
let attacker = app
    .world_mut()
    .spawn((Base::<Life>::new(50.0), Base::<MaxLife>::default()))
    .id();
app.update();

app.world_mut().apply_effect(mage, Hit.source(attacker));
assert_eq!(base(&app, mage), 90.0, "30 to the shield, 10 to life");
assert_eq!(base(&app, attacker), 52.0, "a fifth stolen back");
```

A route's router is [`RouteFn::route`] on its type, and a value of it is
added with [`add_route`], numbered in the [`RouteRegistry`]. An effect
routes into it with the key `route::<IncomingDamage>(amount)`, in place of an
attribute's modifier. An execution routes into the routes it declares on its
derive, by type:

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] struct SpellPower;
# #[derive(Attribute)] struct Resistance;
# #[derive(Route)] struct IncomingDamage;
#[derive(Execution)]
#[execution(source(SpellPower), target(Resistance), routes(IncomingDamage))]
struct FireDamage;

impl ExecutionFn for FireDamage {
    fn execute(&self, effect: &ExecutionContext) -> Vec<BaseChange> {
        let power = effect.captured::<SpellPower>(Side::Source).unwrap_or_default();
        let resisted = effect.captured::<Resistance>(Side::Target).unwrap_or_default();
        vec![BaseChange::route::<IncomingDamage>(30.0 * (1.0 + power / 100.0) * (1.0 - resisted))]
    }
}
# assert_eq!(FireDamage::routes(), vec![Key::of::<IncomingDamage>()]);
```

A router gets all that an effect routed into its route, summed: its
modifiers and executions both. What it reads it [captures](#captures), on
`#[route(..)]` as an execution does: an effect captures them for each route
it routes into, those its modifiers name and its executions declare. A route
whose captures can't be, one of an attribute the app hasn't added, say,
rejects only the effects routing into it. An execution routing into a route
it doesn't declare has nothing captured for its router: the change is
dropped, warned of once. Declaring a route the app hasn't added rejects the
effects running it. Its changes come after the effect's own, and are rounded
and kept within limits like any other.

- **It's the same everywhere.** Routing happens as the effect works out its
  changes, so a [preview](applying-effects.md#previewing) sees it, and an
  ability's cost paid through a route, "blood magic" paying mana costs in
  life, is checked as routed. A client predicting an ability routes its cost
  as the server will, and undoes it if the server refuses.
- **Routing is single-pass.** What a router routes into a route is dropped,
  with a warning. One router can do the whole pipeline.
- **It's routed, not held.** A route takes flat amounts, from effects that
  execute: instant, or periodic. It has no base, limits or rounding, nothing
  holds it, and nothing reads it. What it's routed into is replicated as
  ever.
- **A modifier changes one or the other.** A modifier names an attribute or
  a route, never both, and the registry says so.

[`EffectExecuted`] says what came of it: `changes` to the target's bases
after routing and limits, `source_changes` to its source's, and `routed`,
what it routed into each route. A combat log reads "40 dealt, 30 absorbed"
from them.

## Calculations

A calculation is a `Custom` magnitude. Anywhere a [`Magnitude`] goes, so can
`Magnitude::calculation::<BurnTime>()`: a modifier's value, a duration, a
period. Data writes it as the variant, with the calculation's name,
`"burn_time"`.

When a calculation runs depends on what it stands for:

- A duration or a period is worked out once, as the effect applies.
- An executing modifier's value is worked out each time the effect executes.
- A lasting modifier's value is worked out as the effect starts counting, and
  again whenever its stacks change or its definition is rebuilt.

What a calculation declares it [captures](#captures) is captured with the
effect's other captures: a snapshot is kept, and a live read followed, so a
lasting modifier's calculation is worked out again as what it reads live
changes. What it reads otherwise, with `breakdown` or `world`, is read as
it's worked out.

## Pitfalls

- **An execution declares its routes.** What it routes into a route it
  doesn't declare, `routes(..)` on its derive, is dropped: its router's
  captures weren't taken.
- **A router can't apply effects.** Like executions, it reads the world and
  returns changes. To apply something as a hit lands, observe
  [`EffectExecuted`].
- **Executions need something to run on.** An effect with neither `Instant`
  duration nor a period never executes, so the registry rejects executions
  on it. Use a modifier, or give the effect a period.
- **Code has no side effects.** It reads the world and returns numbers. To
  spawn something or play a sound as an effect executes, observe
  [`EffectExecuted`], checking `is::<E>()` for the effect.
- **Add code as the app is built.** Effects are checked as the app starts,
  against the code the app has then: an effect naming an execution, a
  calculation or a route added later is rejected.
- **Previews run code too.** [`EffectWorld::preview_effect`], which abilities
  use to check a cost, runs executions and calculations without applying
  anything.

[`ExecutionContext`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.ExecutionContext.html
[`BaseChange`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.BaseChange.html
[`Execution`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.Execution.html
[`Calculation`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.Calculation.html
[`ExecutionFn::execute`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.ExecutionFn.html#tymethod.execute
[`CalculationFn::calculate`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.CalculationFn.html#tymethod.calculate
[`RouteFn::route`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.RouteFn.html#tymethod.route
[`add_execution`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectApp.html#tymethod.add_execution
[`add_calculation`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectApp.html#tymethod.add_calculation
[`add_route`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectApp.html#tymethod.add_route
[`EffectApp`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/trait.EffectApp.html
[`ExecutionRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.ExecutionRegistry.html
[`CalculationRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.CalculationRegistry.html
[`RouteRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.RouteRegistry.html
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectExecuted.html
[`EffectWorld::preview_effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.preview_effect
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`ContextData`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/trait.ContextData.html
