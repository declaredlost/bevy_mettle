# How it fits together

The parts of bevy_mettle meet on the actor. This chapter shows what is
code and what is data, who writes what, and in which order.

## Code and data

In GAS an effect is a data asset. In mettle it's code:

- **Code defines.** Attributes, effects, abilities and curves are marker
  types of yours, each defining itself with a derive: `Life` and `Mana`,
  `Burning`, `Fireball`, `FireballDamage`. Each key on an effect's,
  ability's or curve's derive is the builder method of the same name, so
  what one is reads in one place and is checked by the compiler. What an
  ability does is a component of yours, run by your systems, and formulas
  are code too: executions, calculations and routes.
- **Data adds and tunes.** [`MettleContent`] adds tags, curves, effects
  and abilities by keys code doesn't define, for a mod or a server's
  content, and tunes the curves code defines by their keys, as GAS tunes
  curve tables.
- **Context carries what changes.** What differs from one application to
  the next, an amount of damage, who was struck before, goes on the effect's
  spec as context data, `Burn.with(Hurt { amount: 5.0 })`, and a magnitude
  reads it with `Magnitude::context::<Hurt>("amount")`. The effect itself
  stays a marker.

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// How much a burn hurts each time: set as it's applied.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Hurt {
    amount: f32,
}

/// Burning: four seconds, hurting every half second.
#[derive(Effect)]
#[effect(lasting = 4.0, period = 0.5)]
#[effect(flat::<Life>(Magnitude::context::<Hurt>("amount").times(-1.0)))]
struct Burning;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .register_type::<Hurt>()
    .add_effect::<Burning>();
# // Context data is numbered as the app's finished, as `run` does.
# app.finish();
# app.update();
# assert!(app.world().resource::<EffectRegistry>().id_of::<Burning>().is_some());
// Applied as `commands.entity(mage).apply_effect(Burning.with(Hurt { amount: 5.0 }))`.
```

A type's key is its name in `snake_case`, `burning`, unless its derive says
`name = ".."`. [`Key::of::<Burning>()`][`Key`] is that key, as a
`Key<AnyEffect>`; content names the same effect by the string `"burning"`.
A type is of one kind, its key's kind inferred from it, so a misspelled
name, or an ability where an effect goes, doesn't compile.

Code is added to the app: attributes with `add_attribute`, behaviours with
`add_behaviour`, effect code with `add_execution`, `add_calculation` and
`add_route`, context data with `register_type`, and tags, curves, effects
and abilities with `add_tag`, `add_curve`, `add_effect` and `add_ability`.

As the app starts, mettle checks what code defines and content adds as a
whole: an effect naming a tag that isn't defined, an attribute that wasn't
added, a curve that doesn't exist, or content defining a key code already
defines is an error, all of them at once. What holds becomes a
[`TagRegistry`], a [`CurveRegistry`], an [`EffectRegistry`] and an
[`AbilityRegistry`], with or without content. They're built again as
[`MettleContent`] changes, and, with the `hotpatching` feature, as code is
hot patched, each type defining itself again. What doesn't hold is
rejected, and the registries before stay. Mettle doesn't load files. Your
game reads them however it likes, and hands the content over: see
[Adding and tuning content](content.md).

## Names and numbers

Everything mettle runs on is known by a number, as Bevy knows a component by
its `ComponentId`. Names are for people: what data files are written in,
what's shown in debug output, and what a client keys icons and translations
by.

- **Code is numbered as it's added.** `add_attribute::<Life>()` gives `Life`
  the next [`AttributeId`]; behaviours, routes, executions and calculations
  get theirs the same way. Code names each by type: reads and changes,
  such as `value::<Life>()`, `Current<Life>` and `BaseChange::of::<Life>`,
  find its number for you. `world.attribute_id::<Life>()` asks for it, as
  Bevy has `world.component_id::<T>()`, to match what an event names.
- **What's checked is numbered as it's checked.** Tags, curves, effects
  and abilities, whether code defines them or content adds them, get a
  [`TagId`], a [`CurveId`], an [`EffectId`] and an [`AbilityId`], in order
  of name. Building the registries again keeps the numbers things had, and
  numbers new ones after them.
- **Data names code by name.** An effect names the attribute `"life"`, an
  ability the behaviour `"cast_fireball"`. Checking the content resolves
  each name to its number, once: an effect or an ability as the app runs it,
  its `EffectDef` or `AbilityDef`, holds numbers, not names. Code names
  by type, and the type is resolved to its number the same way.
- **The network carries numbers.** A server and its clients run the same
  code and the same content. Adding the same attributes, behaviours and
  routes in the same order numbers them alike, and what's numbered in
  order of name is numbered alike wherever it's checked. Replication already
  asks the same of components.

Each kind of thing has a registry, a resource that keeps the names beside
the numbers: [`AttributeRegistry`], `BehaviourRegistry`,
`ExecutionRegistry`, `CalculationRegistry` and `RouteRegistry` for code;
[`TagRegistry`], [`CurveRegistry`], [`EffectRegistry`] and
[`AbilityRegistry`] for what's checked as a whole. The registries of code, effects and
abilities read the same way: `get(id)` for what's numbered `id`,
`id("name")` for a name's number, `id_of::<T>()` for a type's, `name(id)`
for a number's name, and `iter()` for all of them. `TagRegistry` goes
between a tag and its number with `id` and `key`, and `CurveRegistry` with
`id` and `name`.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins)).add_attribute::<Life>();
let attributes = app.world().resource::<AttributeRegistry>();
let life = attributes.id_of::<Life>().unwrap();
assert_eq!(attributes.id("life"), Some(life));
assert_eq!(attributes.name(life), Some("life"));
```

## Who writes what

| Part | Writes | Reads |
|---|---|---|
| Attributes | `Current<A>`, from bases and modifiers | `Base<A>`, `Modifiers` of the actor's sources, its `OwnedTags` |
| Effects | `Modifiers` and granted tags while they last; bases as they execute | Their registry, attributes, owned tags |
| Abilities | Their behaviour's component; costs and cooldowns as effects | Their registry, attributes, owned tags |
| Your systems | Bases, `OwnedTags`, effects applied, abilities activated | `Current<A>`, abilities' components, what's told |

For the mage, casting the fireball applies the effects mettle made of its
cost and cooldown, `fireball.cost` and `fireball.cooldown`. The cost lowers
`Base<Mana>`, the cooldown tags its owner `cooldown.fireball`, and attributes work out `Current<Mana>` from the new
base.

## One step

Mettle runs in its own schedule, [`Mettle`], once each fixed step, in
`FixedPostUpdate`. Its steps, the [`MettleSet`]s, run in order:

1. **Effects**: requirements weighed, periods come round, time run out.
2. **Attributes**: changed bases synced in, changed modifiers folded, owned
   tags told, values worked out.
3. **Settle**: effects whose live reads moved are worked out again, and
   attributes folded again, until nothing moves.
4. **Publish**: changed values written to `Current<A>`, and told.

So what your systems do in a tick's `FixedUpdate` is ticked, folded and
published before the tick ends. A rollback replays the tick, and mettle
with it. Abilities stand apart: in [`AbilitySystems`], in `PreUpdate`, new
definitions are given out and, over a network, requests served and replies
resolved.

The system running `Mettle` is in [`MettleSystems`]. Order your systems
against it:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# fn regenerate() {}
# fn health_bars() {}
# let mut app = App::new();
app.add_systems(FixedUpdate, regenerate)
    .add_systems(FixedPostUpdate, health_bars.after(MettleSystems));
```

Applying an effect or activating an ability doesn't wait for a schedule.
It happens as the command is applied, and what it changes is folded the
next time `Mettle` runs. [Installation](installation.md#where-mettle-runs)
shows how to run it elsewhere.

## Owned tags

An actor's [`OwnedTags`] are what it's going through: `state.burning`,
`state.stunned`, `cooldown.fireball`. Effects grant them while they last,
abilities while they're active, and your systems add their own, such as
`state.moving`. Every part reads them: a modifier can count only while
its owner owns a tag, an effect can need one to apply, and an ability is
blocked by one. [Gameplay tags](tags.md) covers them.

## Pitfalls

- **Ordering across schedules.** `.after(MettleSystems)` orders systems
  only within the schedule mettle runs in. A system in `Update` sees what
  the frame's last fixed step published.
- **A change made after mettle ran.** A base changed after `MettleSystems`
  reaches `Current` the next time it runs.

[`Key`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Key.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`TagRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRegistry.html
[`CurveRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Mettle.html
[`MettleSet`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.MettleSet.html
[`MettleSystems`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.MettleSystems.html
[`TagId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.TagId.html
[`CurveId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.CurveId.html
[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`AbilityId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AbilityId.html
[`AttributeRegistry`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeRegistry.html
[`AttributeId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.AttributeId.html
