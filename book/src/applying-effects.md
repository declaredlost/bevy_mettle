# Applying effects

An effect's definition says what it does. To use it, you apply a spec of it
to a target: this effect, at this level, from this caster, with these
numbers. Code names the effect by its type, as [Defining
effects](effects.md#naming-effects-in-code) shows: `#[derive(Effect)] struct
Burning;`, added with `app.add_effect::<Burning>()`. A lasting effect then
becomes an entity of its own, attached to its target, until it runs out or
something takes it off.

## Specs

An [`EffectSpec`] is what's applied. It's Unreal's `GameplayEffectSpec`:
which effect, and these fields:

| Field | What it is |
|---|---|
| `level` | What [curves](curves.md) are read at: 1 unless it says |
| `params` | Numbers the applying code sets, for `Param` magnitudes |
| `source` | Who applied it: the caster, the attacker |
| `activation` | The ability activation it's applied under, if any: see [Prediction](prediction.md) |

An effect type is its own spec, at level 1 from no one: `Burning` is
`EffectSpec::of::<Burning>()`. Its builders make a spec with more set. A
param is named by a [tag](tags.md), here `param.damage` as a type:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# let caster = Entity::PLACEHOLDER;
#[derive(Effect)]
struct Burning;

#[derive(Tag)]
struct Param;

#[derive(Tag)]
#[tag(parent = Param, name = "damage")]
struct DamageParam; // "param.damage"

let spec = Burning.level(3.0).param(DamageParam, -5.0).source(caster);
assert_eq!(spec.level, 3.0);
assert_eq!(spec.param_value(DamageParam), Some(-5.0));
assert_eq!(spec.source, Some(caster));
assert_eq!(EffectSpec::from(Burning), EffectSpec::of::<Burning>());
```

Setting a param again replaces it. There's no spec of a name: code that
doesn't know an effect's type, such as a tool listing every effect, names it
by its number, `EffectSpec::from_id(id)`, found with
`registry.id("burning")`. Which effect a spec applies is resolved to its
number as it's applied.

## Applying

Apply a spec through commands, from any system:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component)] struct Mage;
# #[derive(Effect)] struct Burning;
# #[derive(Tag)] struct Param;
# #[derive(Tag)] #[tag(parent = Param, name = "damage")] struct DamageParam;
/// Sets every mage within reach on fire.
fn ignite(mages: Query<Entity, With<Mage>>, mut commands: Commands) {
    for mage in &mages {
        commands
            .entity(mage)
            .apply_effect(Burning.param(DamageParam, -5.0));
    }
}
```

`apply_effect` takes the type itself too: `apply_effect(Burning)`. Or apply
on the world, at once, from an exclusive system or a test, with
[`EffectWorld`]: `world.apply_effect(mage, spec)`.

Applying checks, in order:

1. There's an [`EffectRegistry`], and it has the effect: its type was added
   with `add_effect`, or its number is one content has. If not, nothing
   happens but a warning.
2. The target owns tags. Any actor with an attribute's `Base` does. Without
   [`OwnedTags`], nothing happens but a warning.
3. The effect's `require` holds, and no effect on the target makes it immune.
   If not, [`EffectBlocked`] says why, and nothing else happens.
4. Active effects it `removes` come off.

Then:

- **An instant effect** executes, changing the target's bases, and chains its
  `on_apply` effects.
- **A stacking effect** already on the target gains a stack. [Stacking](stacking.md)
  covers that.
- **Anything else** spawns an active effect. Its duration and period are
  worked out from the spec. One that would last 0 seconds or less, or not a
  number, isn't applied, with a warning. If its `ongoing` requirement holds,
  it starts counting, and executes at once if its period says `on_apply`.
  [`EffectApplied`] is triggered, and its `on_apply` effects are chained.

## The burning mage

This mage burns for 5 every half second, gets wet, and dries off:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0, max = MaxLife)]
# struct Life;
# #[derive(Attribute)]
# #[attribute(base = 100.0)]
# struct MaxLife;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State, name = "burning")] struct OnFire;
# #[derive(Tag)] #[tag(parent = State)] struct Wet;
# #[derive(Tag)] struct Param;
# #[derive(Tag)] #[tag(parent = Param, name = "damage")] struct DamageParam;
# #[derive(Effect)] struct Burning;
# fn tags() -> Vec<TagKey> {
#     [
#         "effect", "effect.affliction", "effect.affliction.burning",
#         "immune", "immune.fire", "param", "param.damage",
#         "state", "state.burning", "state.wet",
#     ]
#     .map(TagKey::named)
#     .to_vec()
# }
# fn burning() -> EffectData {
#     EffectData::lasting("burning", 4.0)
#         .tag("effect.affliction.burning")
#         .period(0.5, true)
#         .modifier(EffectModifier::new::<Life>(Operation::Flat, Magnitude::param("param.damage")))
#         .grant("state.burning")
#         .require(!Condition::owner_has("immune.fire"))
#         .ongoing(!Condition::owner_has("state.wet"))
# }
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .add_effect::<Burning>()
#     .insert_resource(MettleContent { tags: tags(), effects: vec![burning()], ..default() })
#     // One fixed step, 1/64 of a second, each update.
#     .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
# app.finish();
# app.cleanup();
# app.update();
# /// Runs `seconds` of fixed steps.
# fn run(app: &mut App, seconds: f32) {
#     for _ in 0..(seconds * 64.0) as u32 {
#         app.update();
#     }
# }
let mage = app
    .world_mut()
    .spawn((Base::<Life>::default(), Base::<MaxLife>::default()))
    .id();
let life = |app: &App| app.world().get::<Base<Life>>(mage).unwrap().get();

app.world_mut()
    .apply_effect(mage, Burning.param(DamageParam, -5.0));
assert_eq!(life(&app), 95.0, "once as it's applied");

run(&mut app, 1.0);
assert_eq!(life(&app), 85.0, "and twice a second");

// The burn is an entity of its own.
let burn = app.world().get::<ActiveEffects>(mage).unwrap().iter().next().unwrap();
let active = app.world().get::<ActiveEffect>(burn).unwrap();
assert_eq!(active.remaining(), Some(3.0));
assert!(app.world().get::<OwnedTags>(mage).unwrap().contains(OnFire));

// Wet, the burn is inhibited: no damage, and its time stands still.
app.world_mut().get_mut::<OwnedTags>(mage).unwrap().add(Wet);
run(&mut app, 1.0);
let active = app.world().get::<ActiveEffect>(burn).unwrap();
assert!(active.is_inhibited());
assert_eq!(active.remaining(), Some(3.0));
assert_eq!(life(&app), 85.0);
assert!(!app.world().get::<OwnedTags>(mage).unwrap().contains(OnFire));

// Dry again, it burns on for the three seconds it had left.
app.world_mut().get_mut::<OwnedTags>(mage).unwrap().remove(Wet);
run(&mut app, 3.0);
assert_eq!(life(&app), 55.0);
let left = app.world().get::<ActiveEffects>(mage).map_or(0, |effects| effects.iter().count());
assert_eq!(left, 0, "and it's gone");
```

The damage changes the mage's base, so it stays taken after the burn ends.

## Active effects

A lasting effect on its target is an entity with an [`ActiveEffect`] and an
[`EffectOf`] pointing at the target. The target has [`ActiveEffects`], the
list of them, in the order they were applied. These are a Bevy relationship:
despawn the target, and its effects go with it.

[`ActiveEffect`] tells you how it stands:

- `spec()`: what was applied.
- `def()` and `id()`: its definition, and its number, an [`EffectId`].
- `stacks()`: how many stacks of it there are.
- `remaining()`: the seconds it has left, or `None` if it's infinite.
- `is_inhibited()`: whether its `ongoing` requirement doesn't hold.

While it counts, a lasting effect carries its modifiers as
[`Modifiers`] [`ModifierOf`] the target, like an item would, and the target
owns the tags it grants. While it's inhibited, it carries neither, and its
time stands still. A periodic effect never carries modifiers: it executes
instead.

## Ticking

[`EffectPlugin`] ticks active effects in [`EffectSystems`], in `FixedUpdate`
unless you give it another schedule. Each tick, for each effect:

1. If its `remove_if` holds, it comes off.
2. If its `ongoing` requirement stopped holding, it's inhibited. If it holds
   again, the effect counts again.
3. If it's counting, its period comes round as often as it's due, and its
   time runs down.
4. If its time ran out, it expires: it comes off, unless its [stacking](stacking.md)
   says otherwise.

Attributes fold the changes on the next attribute pass. With
[`MettlePlugins`], that's [`MettleSystems`]'s order.

## Removing effects

Take effects off by their tags, or one by one:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
#[effect(name = "fireball.cooldown")]
struct FireballCooldown;

/// Cleanses every affliction off whoever drinks a potion.
fn cleanse(drinkers: Query<Entity, Added<Drinking>>, mut commands: Commands) {
    let afflictions: Tags = ["effect.affliction"].into_iter().collect();
    for drinker in &drinkers {
        commands.entity(drinker).remove_effects(afflictions.clone());
    }
}

/// Ends fireball's cooldown early, as a reset rune does.
fn reset(
    effects: Query<(Entity, &ActiveEffect)>,
    registry: Res<EffectRegistry>,
    mut commands: Commands,
) {
    let cooldown = registry.id_of::<FireballCooldown>();
    for (effect, active) in &effects {
        if Some(active.id()) == cooldown {
            commands.entity(effect).remove_effect();
        }
    }
}
# #[derive(Component)] struct Drinking;
```

`remove_effects` goes on the actor, `remove_effect` on the active effect.
[`EffectWorld`] has the same on the world. Either way, the effect's tags
and modifiers are taken back, [`EffectRemoved`] is triggered, and its
`on_remove` effects are chained.

## What's told

Effects trigger entity events on their target, for cues, sounds and
abilities to observe. Each names its effect by number, `id`, an
[`EffectId`]; compare it with the registry's number for a type,
`registry.id_of::<Burning>()`, or look up its name with
`registry.name(id)` for a log:

| Event | When |
|---|---|
| [`EffectApplied`] | A lasting effect was spawned on the target: `effect` is its entity |
| [`EffectStacked`] | An effect gained or lost a stack: `stacks` is how many now |
| [`EffectBlocked`] | An effect didn't apply: `reason` is `Requirement`, `Immunity` or `StackLimit` |
| [`EffectExecuted`] | An effect changed bases: an instant one applied, or a period came round. `changes` says what it changed the target's bases by, `source_changes` its source's, and `routed` what it dealt each [route](effect-code.md#routes) |
| [`EffectRemoved`] | An active effect came off: `expired` says if it ran out of time |

Observe them for every actor, or for one:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
struct Burning;

#[derive(Effect)]
struct Ward;

/// Plays a sizzle whenever anything starts burning.
fn sizzle(applied: On<EffectApplied>, registry: Res<EffectRegistry>) {
    if registry.id_of::<Burning>() == Some(applied.id) {
        // Play the sound on `applied.entity`.
    }
}

fn spawn_mage(mut commands: Commands) {
    commands
        .spawn(Base::<Life>::default())
        // Only this mage's ward breaking.
        .observe(|removed: On<EffectRemoved>, registry: Res<EffectRegistry>| {
            if registry.id_of::<Ward>() == Some(removed.id) && !removed.expired {
                // Flash the broken ward.
            }
        });
}
# #[derive(Attribute)] #[attribute(base = 100.0)] struct Life;
# App::new().add_observer(sizzle).add_systems(Startup, spawn_mage);
```

## Previewing

[`EffectWorld::preview_effect`] says what applying a spec as an instant
effect would change, without changing it: a list of [`BaseChange`]s. An
ability checks it can pay its cost this way. A tooltip can show what a hit
would do.

## Pitfalls

- **Despawning it yourself is removing it.** Despawn an active effect, or
  take its `ActiveEffect` off, and it's told as removed and its `on_remove`
  chained, as with `remove_effect`, once commands are applied. Effects
  despawned with their target tell nothing: there's no one left to tell.
- **Commands wait.** `apply_effect` on commands applies when commands are
  applied, later in the frame. Read the result on the next system, or use
  `EffectWorld` in an exclusive system.
- **An actor with no attributes owns no tags.** Effects need the target's
  [`OwnedTags`]. Every `Base` brings them; on an actor without one, add
  `OwnedTags` yourself.

[`EffectId`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectId.html
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectWorld`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html
[`EffectWorld::preview_effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.preview_effect
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`EffectPlugin`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectPlugin.html
[`EffectSystems`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSystems.html
[`ActiveEffect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.ActiveEffect.html
[`ActiveEffects`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.ActiveEffects.html
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectApplied`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectApplied.html
[`EffectStacked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectStacked.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
[`EffectExecuted`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectExecuted.html
[`EffectRemoved`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectRemoved.html
[`BaseChange`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/code/struct.BaseChange.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`Modifiers`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.Modifiers.html
[`ModifierOf`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/struct.ModifierOf.html
[`MettlePlugins`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettlePlugins.html
[`MettleSystems`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleSystems.html
