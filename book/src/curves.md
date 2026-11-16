# Curves

A curve gives a value by key. Usually the key is the level: the mage's
fireball hits harder as it levels up, 10 damage at level 1, 85 at level 10,
240 at level 20. Effects scale their numbers by curves, as Unreal's curve
tables do. A magnitude can read a curve at anything else it works out too,
an attribute for diminishing returns: see
[Magnitudes](effects.md#magnitudes).

## Defining a curve

A curve is a marker type that defines itself. Derive [`ScalingCurve`] on a
unit struct and give it its keys as `(at, value)` pairs, rising:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
/// Steps up at levels 10 and 20.
#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)], constant)]
struct FireballDamage;

/// Grows steadily between its keys.
#[derive(ScalingCurve)]
#[curve(key(1.0, 4.0), key(20.0, 40.0))]
struct BurningDamage;

App::new().add_curve::<FireballDamage>().add_curve::<BurningDamage>();
```

Each key on `#[curve(..)]` is the [`CurveData`] builder method of the same
name: `keys = [..]` is `.keys([..])`, `key(at, value)` adds one key,
`constant` is `.constant()`, and `ease = EaseFunction::SineInOut` is
`.ease(EaseFunction::SineInOut)`. A curve's key is its type's name in
`snake_case`, `fireball_damage`, unless `name = ".."` says otherwise. For a
long definition, `define = function` hands it to a
`fn(CurveData) -> CurveData` of yours instead.

Between keys, a curve goes one of three ways:

| Key | Between two keys |
|---|---|
| none | A straight line from one to the next, `Linear`. |
| `constant` | Each key's value, held until the next. |
| `ease = ..` | One of Bevy's easing functions, such as `EaseFunction::SineInOut` or `EaseFunction::ExponentialIn`, from one key to the next. |

Before the first key and past the last, a curve stays at that key's value. A
curve with one key is that value at every level.

An effect names a curve by its type: `Magnitude::scaled::<FireballDamage>(1.0)`
is 1 times the curve at the spec's level, as Unreal's scalable float is.
[Defining effects](effects.md) shows it.

## Tuning in data

Code says which curves there are; data tunes them, as Unreal's curve tables
do. A [`CurveData`] in [`MettleContent`]'s `curves` with a code curve's key
replaces that curve's keys. One with a key no code defines adds a curve,
for a mod's effects to scale by.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(ScalingCurve)]
# #[curve(keys = [(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)], constant)]
# struct FireballDamage;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_curve::<FireballDamage>()
    .insert_resource(MettleContent {
        curves: vec![CurveData::new("fireball_damage").keys([(1.0, 12.0), (20.0, 300.0)])],
        ..Default::default()
    });
app.update();

let curves = app.world().resource::<CurveRegistry>();
let fireball = curves.id_of::<FireballDamage>().unwrap();
assert_eq!(curves.at(fireball, 1.0), Some(12.0));
```

`CurveData` is a serde type: in a file, it's a struct with a `name` (a
string), an `interpolation`, and `keys` as a list of pairs.
`interpolation` can be left out: it's `Linear` unless it says, and
otherwise `Constant` or `Ease(SineInOut)`. A tuned curve is the data's
alone: it doesn't keep the code curve's interpolation.

## The curve registry

Mettle builds the registry as the app starts, from the curves code adds and
those content adds or tunes, and keeps it as a resource,
`Res<CurveRegistry>`. It builds it again as [`MettleContent`] changes, and,
with the `hotpatching` feature, as code is hot patched, so a curve's keys
can be changed while the game runs.

A curve's number is a [`CurveId`], given in order of name, as Bevy gives a
component its `ComponentId`. Effects scale by the number once content is
checked; the name is for content and debug output. `id_of::<C>()` finds a
curve type's number, [`CurveRegistry::id`] a name's, and
[`CurveRegistry::at`] reads it:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(ScalingCurve)]
# #[curve(key(1.0, 4.0), key(20.0, 40.0))]
# struct BurningDamage;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_curve::<BurningDamage>();
app.update();

let curves = app.world().resource::<CurveRegistry>();
let burning = curves.id_of::<BurningDamage>().unwrap();
assert_eq!(curves.id("burning_damage"), Some(burning));
assert_eq!(curves.at(burning, 1.0), Some(4.0));
assert_eq!(curves.at(burning, 50.0), Some(40.0)); // Past the last key.
assert_eq!(curves.id("frost_damage"), None);
```

As the registry is built again, curves keep the numbers they had, and new
ones are numbered after them. [`CurveRegistry::build`] builds one from data
alone, for tools and tests.

Building reports every mistake together:

- **A curve defined twice** in content.
- **A curve with no keys.**
- **A key that isn't finite**, such as `NaN`.
- **Levels that don't rise.** Each key's level has to be above the one
  before it.

An effect added by content names curves by name. As the registries are built,
every curve an effect names is checked against the registry, and resolved
to its number.

## As one of Bevy's curves

[`CurveRegistry::curve`] hands back the built curve itself, as Bevy's
[`Curve<f32>`]. Use it to draw a graph in your tooling, or with Bevy's curve
adaptors:

```rust
# use bevy_mettle::prelude::*;
# let curves = CurveRegistry::build(&[CurveData::new("burning_damage").keys([(1.0, 4.0), (20.0, 40.0)])])
#     .unwrap();
let burning = curves.curve(curves.id("burning_damage").unwrap()).unwrap();
let graph: Vec<f32> = (1..=20).map(|level| burning.sample_clamped(level as f32)).collect();
assert_eq!(graph.len(), 20);
```

## Pitfalls

- **Levels are numbers, not steps.** A level can be 2.5. Each curve goes
  between its keys as it says, so a `constant` curve is the way to get steps.
- **Easing goes from one key to the next.** `ease = EaseFunction::QuadraticIn`
  starts slow after every key, not once across the whole curve.
- **Tuning by the right key.** Data tunes a curve by its key, the type's
  name in `snake_case`. A misspelled key adds a new curve that nothing
  scales by, and leaves the code curve as it is.

[`ScalingCurve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/trait.ScalingCurve.html
[`CurveData`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveData.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`CurveRegistry::build`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.build
[`CurveRegistry::at`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.at
[`CurveId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.CurveId.html
[`CurveRegistry::id`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.id
[`CurveRegistry::curve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.curve
[`Curve<f32>`]: https://docs.rs/bevy/latest/bevy/curve/trait.Curve.html
