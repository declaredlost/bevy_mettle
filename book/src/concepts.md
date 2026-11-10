# How it fits together

The parts of bevy_mettle meet on the actor. This chapter shows what is
data and what is code, who writes what, and in which order.

## Data and code

Mettle splits a game's gameplay in two:

- **Code** is what your systems read: attributes are types of yours,
  `Life` and `Mana`, so a system asks for `Current<Life>`. What an ability
  does is a component of yours, run by your systems.
- **Data** is content: gameplay tags, curves, effects and abilities. Each
  is named by a [`Name`], such as `"burning"` or `"fireball"`, so a designer
  or a mod can add one without code.

Code names things by type, never by string. A tag code uses is a
`#[derive(Tag)]` type, an effect it applies a `#[derive(Effect)]` type, an
ability it grants a `#[derive(Ability)]` type. A misspelled one doesn't
compile. Each type carries its content's name, checked once, as content
loads.

Code is added to the app: attributes with `add_attribute`, behaviours with
`add_behaviour`, effect code with `add_execution`, `add_calculation` and
`add_route`, and the tags, effects and abilities code names with `add_tag`,
`add_effect` and `add_ability`.
Data is handed over as one resource, [`MettleContent`], and checked as a
whole against the code and itself: an effect naming a tag that isn't
defined, an attribute that wasn't added, a curve that doesn't exist, or an
effect code names that content doesn't define is an error, all of them at
once. Content that holds becomes a [`TagRegistry`], a
[`CurveRegistry`], an [`EffectRegistry`] and an [`AbilityRegistry`]; content
that doesn't is rejected, and the content before stays. Mettle doesn't load
files. Your game reads them however it likes, and hands the content over:
see [Loading data](loading-data.md).

## Names and numbers

Everything mettle runs on is known by a number, as Bevy knows a component by
its `ComponentId`. Names are for people: what data files are written in,
what's shown in debug output, and what a client keys icons and translations
by.

- **Code is numbered as it's added.** `add_attribute::<Life>()` gives `Life`
  the next [`AttributeId`]; behaviours, routes, executions and calculations
  get theirs the same way. Ask for one with `world.attribute_id::<Life>()`,
  as Bevy has `world.component_id::<T>()`. Typed reads and changes, such
  as `value::<Life>()`, `Current<Life>` and `BaseChange::of::<Life>`, find
  it for you.
- **Content is numbered as it's checked.** Tags, curves, effects and
  abilities get a [`TagId`], a [`CurveId`], an [`EffectId`] and an
  [`AbilityId`], in order of name. A reload keeps the numbers things had,
  and numbers new ones after them.
- **Data names code by name.** An effect names the attribute `"life"`, an
  ability the behaviour `"cast_fireball"`. Checking the content resolves
  each name to its number, once: an effect or an ability as the app runs it,
  its `EffectDef` or `AbilityDef`, holds numbers, not names. Code names
  content by type, and the type is resolved to its number the same way.
- **The network carries numbers.** A server and its clients run the same
  code and the same content. Adding the same attributes, behaviours and
  routes in the same order numbers them alike, and content numbered in
  order of name is numbered alike wherever it's checked. Replication already
  asks the same of components.

Each kind of thing has a registry, a resource that keeps the names beside
the numbers: [`AttributeRegistry`], `BehaviourRegistry`,
`ExecutionRegistry`, `CalculationRegistry` and `RouteRegistry` for code;
[`TagRegistry`], [`CurveRegistry`], [`EffectRegistry`] and
[`AbilityRegistry`] for content. The registries of code, effects and
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

For the mage, casting the fireball applies `fireball.cost` and
`fireball.cooldown`. The cost lowers `Base<Mana>`, the cooldown grants
`cooldown.fireball`, and attributes work out `Current<Mana>` from the new
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

[`Name`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Name.html
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
