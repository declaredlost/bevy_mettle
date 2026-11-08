# How it fits together

The parts of bevy_mettle meet on the actor. This chapter shows what is
data and what is code, who writes what, and in which order.

## Data and code

Mettle splits a game's gameplay in two:

- **Code** is what your systems read: attributes are types of yours,
  `Life` and `Mana`, so a system asks for `Current<Life>`. What an ability
  does is a component of yours, run by your systems.
- **Data** is content: gameplay tags, curves, effects and abilities. Each
  is named by an [`Id`], such as `"burning"` or `"fireball"`, so a designer
  or a mod can add one without code.

Code is added to the app: attributes with `add_attribute`, behaviours with
`add_behaviour`, effect code with `add_execution` and `add_calculation`.
Data is handed over as one resource, [`MettleContent`], and checked as a
whole against the code and itself: an effect naming a tag that isn't in the
tree, an attribute that wasn't added, or a curve that doesn't exist is an
error, all of them at once. Content that holds becomes a [`TagTree`], a
[`CurveTable`], an [`EffectRegistry`] and an [`AbilityRegistry`]; content
that doesn't is rejected, and the content before stays. Mettle doesn't load
files. Your game reads them however it likes, and hands the content over.
[Loading data](loading-data.md) shows one way.

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

## One frame

Each part runs in its own set, in its own schedule by default:

1. **Abilities**, in [`AbilitySystems`] in `PreUpdate`: new definitions
   given out, and, over a network, requests served and replies resolved.
2. **Attributes**, in [`AttributeSystems`] in `FixedPreUpdate`: changed
   bases synced in, changed modifiers folded, owned tags told, and changed
   values written to `Current<A>`.
3. **Effects**, in [`EffectSystems`] in `FixedUpdate`: requirements
   weighed, periods come round, time run out.

Attributes run before your fixed-step systems, so those read values as
they stand. What effects change in a tick folds into `Current` at the
start of the next.

[`MettleSystems`] holds all three, in each schedule they run in. Where
two run in the same schedule, abilities go first, then effects, then
attributes, so what changed folds the same tick. Order your systems
against it:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# fn regenerate() {}
# fn cast_fireball() {}
# let mut app = App::new();
app.add_systems(FixedUpdate, regenerate.before(MettleSystems))
    .add_systems(FixedUpdate, cast_fireball.after(MettleSystems));
```

Applying an effect or activating an ability doesn't wait for a schedule.
It happens as the command is applied, and what it changes is folded the
next time attributes run.

## Owned tags

An actor's [`OwnedTags`] are what it's going through: `state.burning`,
`state.stunned`, `cooldown.fireball`. Effects grant them while they last,
abilities while they're active, and your systems add their own, such as
`state.moving`. Every part reads them: a modifier can count only while
its owner owns a tag, an effect can need one to apply, and an ability is
blocked by one. [Gameplay tags](tags.md) covers them.

## Pitfalls

- **Ordering across schedules.** `.after(MettleSystems)` orders systems
  only within one schedule. To read attributes the tick effects changed
  them, keep effects and attributes in the same schedule.
- **A change made after attributes ran.** A base changed after
  `AttributeSystems` reaches `Current` the next time they run.

[`Id`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Id.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`TagTree`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagTree.html
[`CurveTable`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveTable.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`AttributeSystems`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributeSystems.html
[`EffectSystems`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSystems.html
[`MettleSystems`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleSystems.html
