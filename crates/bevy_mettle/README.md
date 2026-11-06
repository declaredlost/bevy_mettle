# bevy_mettle

A gameplay framework for Bevy in the spirit of Unreal's Gameplay Ability
System: attributes, effects and abilities. Data first, like GAS:
attributes are the game's own types, each a marker with its rules
derived; content (gameplay tags, curves, effects, abilities) is data,
named by tags and ids, and checked as registries are built from it.
Mettle doesn't load anything: the game reads its files and hands the
definitions over.

| Crate | What it holds | Feature |
|---|---|---|
| `bevy_mettle_core` | Gameplay tags and owned tags, curve tables, `Condition` | always |
| `bevy_mettle_attribute` | Attributes: `Base` and `Current`, modifiers, reads | `attribute` (default) |
| `bevy_mettle_effect` | Effects: instant, timed, infinite, periodic; stacking, granted tags, immunity | `effect` (default) |
| `bevy_mettle_ability` | Abilities: granted, activated, paid for, cooled down; behaviours in code | `ability` (default) |

This crate re-exports each as a module ([`core`], `attribute`, `effect`,
`ability`), and one [`prelude`].

```rust,ignore
use bevy_mettle::prelude::*;

#[derive(Attribute)]
#[attribute(base = 50.0, min = 0.0, max = MaxLife)]
pub struct Life;

app.add_attribute::<Life>()
    .add_plugins(EffectPlugin::default())
    .insert_resource(EffectRegistry::build(&effects, &tags, &curves, &attributes, &code)?);

commands.spawn((Base::<Life>::default(), Base::<MaxLife>::default()));
commands.entity(hero).apply_effect(EffectSpec::new("burning").param("param.damage", 12.0));
```
