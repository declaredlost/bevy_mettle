# Curves

A curve gives a value by key. Usually the key is the level: the mage's
fireball hits harder as it levels up, 10 damage at level 1, 85 at level 10,
240 at level 20. Effects scale their numbers by curves, as Unreal's curve
tables do. A magnitude can read a curve at anything else it works out too,
an attribute for diminishing returns: see
[Magnitudes](effects.md#magnitudes).

## Defining a curve

A curve is data, a [`CurveData`]: a name, how it goes between its keys, and
its keys as `(at, value)` pairs, rising:

```rust
# use bevy_mettle::prelude::*;
fn curves() -> Vec<CurveData> {
    vec![
        CurveData {
            name: "fireball.damage".into(),
            interpolation: Interpolation::Constant,
            keys: vec![(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)],
        },
        CurveData {
            name: "burning.damage".into(),
            interpolation: Interpolation::Linear,
            keys: vec![(1.0, 4.0), (20.0, 40.0)],
        },
    ]
}
# assert_eq!(curves().len(), 2);
```

Here the fireball's damage steps up at levels 10 and 20, and burning's
grows steadily between its keys. `CurveData` is a serde type: in a file, it's
a struct with a `name` (a string), an `interpolation`, and
`keys` as a list of pairs. `interpolation` can be left out: it's `Linear`
unless it says.

Between keys, a curve goes one of three ways:

| `interpolation` | Between two keys |
|---|---|
| `Linear` | A straight line from one to the next. The default. |
| `Constant` | Each key's value, held until the next. |
| `Ease(..)` | One of Bevy's easing functions, such as `Ease(EaseFunction::SineInOut)` or `Ease(EaseFunction::ExponentialIn)`, from one key to the next. |

Before the first key and past the last, a curve stays at that key's value. A
curve with one key is that value at every level.

## The curve registry

In a game, the curves go in [`MettleContent`]'s `curves`, and mettle builds
the registry as the content is loaded, keeping it as a resource,
`Res<CurveRegistry>`. [`CurveRegistry::build`] is that step on its own: it checks
each curve, builds it as one of Bevy's curves, and numbers it.

A curve's number is a [`CurveId`], given in order of name, as Bevy gives a
component its `ComponentId`. Effects scale by the number once content is
checked; the name is for content and debug output. [`CurveRegistry::id`] finds
a curve's number by its name:

```rust
# use bevy_mettle::prelude::*;
# fn curves() -> Vec<CurveData> {
#     vec![
#         CurveData {
#             name: "fireball.damage".into(),
#             interpolation: Interpolation::Constant,
#             keys: vec![(1.0, 10.0), (10.0, 85.0), (20.0, 240.0)],
#         },
#         CurveData {
#             name: "burning.damage".into(),
#             interpolation: Interpolation::Linear,
#             keys: vec![(1.0, 4.0), (20.0, 40.0)],
#         },
#     ]
# }
let curves = CurveRegistry::build(&curves()).unwrap();

let burning = curves.id("burning.damage").unwrap();
assert_eq!(curves.at(burning, 1.0), Some(4.0));
assert_eq!(curves.at(burning, 50.0), Some(40.0)); // Past the last key.
assert_eq!(curves.id("frost.damage"), None);
```

As content is loaded again, the registry is rebuilt with
[`CurveRegistry::rebuild`]: curves keep the numbers they had, and new ones are
numbered after them.

`build` reports every mistake together:

- **A curve defined twice.**
- **A curve with no keys.**
- **A key that isn't finite**, such as `NaN`.
- **Levels that don't rise.** Each key's level has to be above the one
  before it.

Effects name curves by name. As content is loaded, every curve an effect
names is checked against the registry, and resolved to its number. [Defining
effects](effects.md) shows a magnitude scaled by a curve.

## As one of Bevy's curves

[`CurveRegistry::curve`] hands back the built curve itself, as Bevy's
[`Curve<f32>`]. Use it to draw a graph in your tooling, or with Bevy's curve
adaptors:

```rust
# use bevy_mettle::prelude::*;
# let curves = CurveRegistry::build(&[CurveData {
#     name: Key::from_static("burning.damage"),
#     interpolation: Interpolation::Linear,
#     keys: vec![(1.0, 4.0), (20.0, 40.0)],
# }])
# .unwrap();
let burning = curves.curve(curves.id("burning.damage").unwrap()).unwrap();
let graph: Vec<f32> = (1..=20).map(|level| burning.sample_clamped(level as f32)).collect();
assert_eq!(graph.len(), 20);
```

## Pitfalls

- **Levels are numbers, not steps.** A level can be 2.5. Each curve goes
  between its keys as it says, so a `Constant` curve is the way to get steps.
- **Easing goes from one key to the next.** `Ease(QuadraticIn)` starts slow
  after every key, not once across the whole curve.

[`CurveData`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveData.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`CurveRegistry::build`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.build
[`CurveId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.CurveId.html
[`CurveRegistry::id`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.id
[`CurveRegistry::rebuild`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.rebuild
[`CurveRegistry::curve`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html#method.curve
[`Curve<f32>`]: https://docs.rs/bevy/latest/bevy/curve/trait.Curve.html
