# Stacking

Apply burning to the mage twice, and by default the mage has two burns, each
with its own time. Often you want something else: one burn that gets
stronger, up to a limit, and does something worse once it's at the limit.
That's stacking.

## Stacks of burning

Burning stacks up to three times. A fourth application ignites the mage
instead. Its `stacking` sets a limit of 3, has a stack run out one at a
time, and applies `ignited` on overflow. In data, a stacking can say just
those three, and the rest take their defaults.

A stacking effect applied to a target that already has it doesn't spawn a
second active effect. The one there gains a stack, and its amounts count once
per stack:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Life;
# fn tags() -> Vec<TagKey> {
#     ["effect", "effect.affliction", "effect.affliction.burning", "param", "param.damage"]
#         .map(TagKey::named)
#         .to_vec()
# }
fn effects() -> Vec<EffectData> {
    vec![
        EffectData::lasting("burning", 4.0)
            .tag("effect.affliction.burning")
            .period(0.5, false)
            .modifier(EffectModifier::new::<Life>(
                Operation::Flat,
                Magnitude::param("param.damage"),
            ))
            .stacking(Stacking {
                by: StackBy::Target,
                limit: 3,
                reapply: Reapply::Refresh,
                reset_period: false,
                expire: Expire::RemoveOne,
                overflow: vec!["ignited".into()],
            }),
        EffectData::instant("ignited")
            .modifier(EffectModifier::new::<Life>(Operation::Flat, -20.0)),
    ]
}
# #[derive(Effect)] struct Burning;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_effect::<Burning>()
#     .insert_resource(MettleContent { tags: tags(), effects: effects(), ..default() })
#     .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
# app.finish();
# app.cleanup();
# app.update();
# fn run(app: &mut App, seconds: f32) {
#     for _ in 0..(seconds * 64.0) as u32 {
#         app.update();
#     }
# }
# let mage = app.world_mut().spawn(Base::<Life>::default()).id();
# let life = |app: &App| app.world().get::<Base<Life>>(mage).unwrap().get();
let burn = Burning.param("param.damage", -2.0);
for _ in 0..4 {
    app.world_mut().apply_effect(mage, burn.clone());
}
let effects = app.world().get::<ActiveEffects>(mage).unwrap();
assert_eq!(effects.iter().count(), 1, "one burn");
let burning = effects.iter().next().unwrap();
assert_eq!(app.world().get::<ActiveEffect>(burning).unwrap().stacks(), 3);
assert_eq!(life(&app), 80.0, "the fourth ignited the mage");

run(&mut app, 0.5);
assert_eq!(life(&app), 74.0, "2 damage, three times over");

// Four seconds in, the burn loses a stack and starts its time over.
run(&mut app, 3.5);
let active = app.world().get::<ActiveEffect>(burning).unwrap();
assert_eq!(active.stacks(), 2);
assert_eq!(active.remaining(), Some(4.0));
```

## Settings

A [`Stacking`] has a `limit`, which has to be more than 0. Everything else has
a default:

| Field | What it does | Unless it says |
|---|---|---|
| `by` | Whose applications stack together: `Target`, everyone's on the target, or `Source`, each source's apart | `Target` |
| `limit` | The most stacks | required |
| `reapply` | What a new application does to the time left: `Refresh` starts it over, `Keep` leaves it | `Refresh` |
| `reset_period` | Whether a new application starts the period over | `false` |
| `expire` | What running out of time does: `ClearAll`, `RemoveOne`, or `Refresh` | `ClearAll` |
| `overflow` | Effects applied in its place when it's at its limit | none |

With `by` set to `Source`, two mages burning the same target each have their own
stacks, each its own active effect. Set to `Target`, they share one.

## A new stack

An application that stacks:

- adds a stack;
- starts the time over, if `reapply` is `Refresh`;
- starts the period over, if `reset_period` is set;
- works the effect's modifiers out again for the new count;
- triggers [`EffectStacked`] with how many stacks there are now;
- chains the effect's `on_apply` effects.

At the limit, an application adds no stack. It still starts the time and the
period over as the settings say. Then it applies the `overflow` effects, with
the same spec, and triggers [`EffectBlocked`] with the reason `StackLimit`.
The effect's own `on_apply` effects aren't chained.

## How stacks count

Each stack counts the effect's amounts once more. Three stacks of +10%
damage are +30%. Three stacks of burning do three times the damage each
period. A `Base` or an `Override` is a value, not an amount, so it counts
once however many stacks there are.

The active effect keeps the spec it was first applied with. A later
application adds a stack, not its own level, params or source: a stronger
second burn makes three stacks of the first burn's damage. Its
[captures](effects.md#captures) are likewise taken once, as the effect is
first applied, unless they're read live.

[Effects in code](effect-code.md) see the count as `stacks` on their
context, and scale by it themselves.

## Running out

When a stacked effect's time runs out, `expire` decides:

- **`ClearAll`** takes it off, every stack at once. [`EffectRemoved`] says it
  expired, and its `on_remove` and `on_expire` effects are chained.
- **`RemoveOne`** takes one stack off and starts the time over for the rest,
  telling [`EffectStacked`]. The last stack comes off as `ClearAll` would.
- **`Refresh`** starts the time over and keeps every stack. The effect then
  stays until something takes it off.

## Pitfalls

- **Instant effects don't stack.** They don't last, so there's nothing to
  stack onto. The registry rejects an instant effect with stacking.
- **Without stacking, every application is its own effect.** Five
  applications of an effect without `stacking` are five active effects, each
  with its own time.
- **A stack of a stronger spec is still the first one's.** If later
  applications should be able to replace a weaker effect, give it no
  stacking, and have the stronger one `removes` the weaker one's tag.

[`Stacking`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.Stacking.html
[`EffectStacked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectStacked.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
[`EffectRemoved`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectRemoved.html
