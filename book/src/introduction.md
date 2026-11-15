# Introduction

bevy_mettle is a gameplay framework for [Bevy](https://bevy.org), in the
spirit of Unreal's Gameplay Ability System. It gives your actors
attributes, such as life and mana. Effects change them: a fire that
burns, a potion that heals, a ring that adds resistance. Abilities are
what an actor can do: cast a fireball, paid for in mana and cooled down.

This book builds one actor from start to finish: a fire mage. Here is the
mage's fireball, after the first few chapters:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

#[derive(Tag)]
struct State;

#[derive(Tag)]
#[tag(parent = State)]
struct Stunned;

/// What the fireball does while it's cast: a component of yours.
#[derive(Component, Default, Behaviour)]
struct CastFireball;

/// The fireball: a type that defines itself.
#[derive(Ability)]
#[ability(behaviour::<CastFireball>, cost = Cost::of::<Mana>(20.0), cooldown = 2.0)]
#[ability(require = !Condition::owner_has(Stunned))]
struct Fireball;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Mana>()
#     .add_tag::<Stunned>()
#     .add_behaviour::<CastFireball>()
#     .add_ability::<Fireball>();
# app.update();
# let effects = app.world().resource::<EffectRegistry>();
# assert!(effects.id("fireball.cost").is_some());
# assert!(effects.id("fireball.cooldown").is_some());
# assert!(app.world().resource::<AbilityRegistry>().id_of::<Fireball>().is_some());
```

Each key on `#[ability(..)]` is a call to the builder of the same name, so
the fireball is all in one place, checked by the compiler. The mage casts it
with `commands.entity(mage).activate_ability(Fireball)`. That pays 20 mana,
starts a two-second cooldown, and puts `CastFireball` on the ability for
your systems to run. Mettle makes the cost and the cooldown into effects of
their own, `fireball.cost` and `fireball.cooldown`, and tags the mage
`cooldown.fireball` while it cools down. While the mage is stunned, or the
cooldown lasts, or there isn't enough mana, it fails, and says why.

## What you get

- **Attributes are your types.** `Life`, `Mana` and `Resistance` are marker
  types with their rules derived: a base, limits, rounding. Each actor keeps
  a `Base` and a `Current` of each as components.
- **Modifiers.** Flat, increased, more, overrides and the rest, from items,
  effects or the actor itself. They can count only in context, such as
  resistance against fire, and you can ask why a value is what it is.
- **Effects.** Instant, timed, infinite or periodic. They stack, tag their
  owner, need conditions to apply or to count, block other effects,
  and chain into each other.
- **Abilities.** Granted, activated, paid for, cooled down and ended. What one
  costs and what blocks it are keys on its type; what it does is a component
  and systems of yours.
- **Gameplay tags.** A tree of labels, such as `damage.elemental.fire`,
  for what effects and abilities are and for what an actor is going through.
- **Code first.** Effects, abilities and curves are types that define
  themselves, where in GAS they're data assets. Data adds content, a mod's
  effects or a server's abilities, and tunes curves, as GAS tunes curve
  tables. What code defines and data adds is checked as a whole as the app
  starts, again as the data changes, and, with the `hotpatching` feature, as
  code is hot patched. Mettle doesn't load files: your game reads them
  however it likes.
- **Prediction.** A client predicts its own abilities and the server
  decides, with an adapter for [lightyear](https://github.com/cBournhonesque/lightyear).

## The crates

Most projects only depend on `bevy_mettle`. It re-exports the other crates
behind feature flags.

| Crate | Contains |
|---|---|
| `bevy_mettle` | Every part, with one prelude and one plugin group |
| `bevy_mettle_core` | Gameplay tags, curves and conditions |
| `bevy_mettle_attribute` | Attributes, modifiers and reads in context |
| `bevy_mettle_effect` | Effects |
| `bevy_mettle_ability` | Abilities |
| `bevy_mettle_lightyear` | Prediction over lightyear |
| `bevy_mettle_macros` | The derives: `Attribute`, `Tag`, `Effect`, `Ability`, `Behaviour` and the rest |

This book is the guide. The API reference is on
[docs.rs](https://docs.rs/bevy_mettle).
