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

/// What the fireball does while it's cast: a component of yours.
#[derive(Component, Default)]
struct CastFireball;

fn fireball() -> Ability {
    Ability::new("fireball", "cast_fireball")
        .tag("ability.spell.fire")
        .cost("fireball.cost")
        .cooldown("fireball.cooldown")
        .blocked_by("state.stunned")
}

fn effects() -> [Effect; 2] {
    [
        Effect::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
        Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
    ]
}
```

The mage casts it with `commands.entity(mage).activate_ability("fireball")`.
That pays 20 mana, starts a two-second cooldown, and puts `CastFireball`
on the ability for your systems to run. While the mage is stunned, or the
cooldown lasts, or there isn't enough mana, it fails, and says why.

## What you get

- **Attributes are your types.** `Life`, `Mana` and `Resistance` are marker
  types with their rules derived: a base, limits, rounding. Each actor keeps
  a `Base` and a `Current` of each as components.
- **Modifiers.** Flat, increased, more, overrides and the rest, from items,
  effects or the actor itself. They can count only in context, such as
  resistance against fire, and you can ask why a value is what it is.
- **Effects.** Instant, timed, infinite or periodic. They stack, grant tags,
  need conditions to apply or to count, make actors immune to other effects,
  and chain into each other.
- **Abilities.** Granted, activated, paid for, cooled down and ended. What one
  costs and what blocks it is data; what it does is a component and systems
  of yours.
- **Gameplay tags.** A tree of labels, such as `damage.elemental.fire`,
  for what effects and abilities are and for what an actor is going through.
- **Data first.** Tags, curves, effects and abilities are data, handed over
  as one resource and checked as a whole, reloaded as it changes. Mettle
  doesn't load files: your game reads them however it likes.
- **Prediction.** A client predicts its own abilities and the server
  decides, with an adapter for [lightyear](https://github.com/cBournhonesque/lightyear).

## The crates

Most projects only depend on `bevy_mettle`. It re-exports the other crates
behind feature flags.

| Crate | Contains |
|---|---|
| `bevy_mettle` | Every part, with one prelude and one plugin group |
| `bevy_mettle_core` | Gameplay tags, curve tables and conditions |
| `bevy_mettle_attribute` | Attributes, modifiers and reads in context |
| `bevy_mettle_effect` | Effects |
| `bevy_mettle_ability` | Abilities |
| `bevy_mettle_lightyear` | Prediction over lightyear |
| `bevy_mettle_macros` | The `Attribute` and `IdTarget` derives |

This book is the guide. The API reference is on
[docs.rs](https://docs.rs/bevy_mettle).
