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
| `context` | What else it carries: who applied it, what struck, where. See [Context](#context) |
| `snapshot` | Its source's attributes as it was made, if the code making it took them: see [Snapshots](#snapshots) |
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
assert_eq!(spec.context.source, Some(caster));
assert_eq!(EffectSpec::from(Burning), EffectSpec::of::<Burning>());
```

Setting a param again replaces it. A spec names its effect by type, never by
name or number: an effect code applies has a type. Which effect a spec
applies is resolved to its number as it's applied.

## Context

A spec's [`EffectContext`] is what the applying code knew beyond numbers.
It's Unreal's `FGameplayEffectContext`. Its one field is `source`, who's
responsible: the archer. Source tags, attribute captures and stacking by
source read it, set with `.source(archer)`.

Everything else is *context data*, one of each type, added with
`.with(..)` and read with `get::<T>()`. Mettle's own are:

| Type | What it is |
|---|---|
| [`Causer`] | What struck: the arrow, the trap. Abilities set their actor |
| [`SourceObject`] | What it came from: the bow, an item |
| [`AbilityRef`] | The ability that applied it: its id, and its level as it was activated. Set by abilities for what they apply |
| [`Hit`] | Where it hit: the point, and the surface's normal |
| [`Origin`] | Where it came from: what knockback pushes away from |

Which run of an ability applied it is the spec's `activation`.

A game adds its own with a [`ContextData`] type, Unreal's context
subclassed. It's for **structured** data, which none of the rest says:

- A flag, a critical or a headshot, is a [tag](tags.md), which conditions,
  blocking and cues see.
- A number is a [param](#specs).
- What struck and what it came from are [`Causer`] and [`SourceObject`].

What's left is a game's own shape: the targets a chain of lightning already
struck, say. Any type deriving `Reflect`, `Clone`, `PartialEq`,
`Serialize`, `Deserialize` and `MapEntities` is context data. Reflect it as
`ContextData`, and register it with the app, as Bevy registers a component:

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# let (mage, bolt) = (Entity::PLACEHOLDER, Entity::PLACEHOLDER);
# let struck = vec![Entity::PLACEHOLDER];
# use bevy_mettle::effect::context::ContextDataRegistry;
#[derive(Effect)]
struct ChainLightning;

/// Who the lightning already struck, for the next jump to skip.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Chain {
    #[entities]
    struck: Vec<Entity>,
}

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .register_type::<Chain>();
app.finish();
let registry = app.world().resource::<ContextDataRegistry>();
assert!(registry.id_of::<Chain>().is_some());

let spec = ChainLightning
    .source(mage)
    .with(Causer(bolt))
    .with(Hit { point: Vec3::new(2.0, 1.5, 0.0), normal: Vec3::NEG_X })
    .with(Chain { struck: struck.clone() });
assert_eq!(spec.context.get::<Causer>(), Some(&Causer(bolt)));
assert!(spec.context.get::<Chain>() == Some(&Chain { struck }));
```

One of each type goes on a context, read by type. Executions and
calculations read it from their [view](effect-code.md#what-code-sees):
`effect.get::<Causer>()`, `effect.get::<Hit>()`, `effect.get::<Chain>()`.
An effect it chains carries it too, and the cues it plays are told it.

It goes with an effect's replica to clients, and with a one-shot cue's,
entities mapped, those in a game's type too. A type goes by its number in
the [`ContextDataRegistry`], built as the app finishes from every type
registered as context data, in order of type path: a server and its clients
number it the same, whatever order they register it in. A type that isn't
registered stays in the world that applied it. A game type is serialized
compactly, as lightyear's messages are, so serde's `skip_serializing_if`,
`flatten` and `untagged` don't round-trip in it.

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
4. Active effects it `ends` end.

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
#     EffectData::lasting(Key::of::<Burning>(), 4.0)
#         .tag("effect.affliction.burning")
#         .period(0.5, true)
#         .modifier(EffectModifier::new::<Life>(Operation::Flat, Magnitude::param("param.damage")))
#         .owner_tag("state.burning")
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

## Snapshots

A fireball takes a second to land. It should hit with its caster's spell
power as it was cast: not more for a buff gained in flight, and not nothing
for a caster who died. Take the spec's snapshot as it's made, and carry the
spec:

- [`EffectWorld::snapshot`] on the world: `world.snapshot(spec)`.
- [`Snapshots`], a system param: `snapshots.take(spec)`.

Either reads, from the spec's source, every [capture](effects.md#captures)
of its effect that's a snapshot of the source's, and keeps them in the
spec's `snapshot`. Applied later, it uses them, not the source as it is
then. A spec applied without one takes its snapshot as it's applied, and
so does a capture its source couldn't give yet (an attribute it doesn't
have). The target's captures are taken as it's applied, and live reads are
read as they're needed, either way.

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
#[derive(Attribute)]
struct SpellPower;

#[derive(Effect)]
#[effect(name = "fireball.hit")]
struct FireballHit;

/// As much damage as the caster has spell power.
fn hit() -> EffectData {
    EffectData::instant(Key::of::<FireballHit>()).modifier(EffectModifier::new::<Life>(
        Operation::Flat,
        Magnitude::source::<SpellPower>().times(-1.0),
    ))
}

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .add_attribute::<Life>()
    .add_attribute::<SpellPower>()
    .add_effect::<FireballHit>()
    .insert_resource(MettleContent { effects: vec![hit()], ..default() });
app.update();
let caster = app.world_mut().spawn(Base::<SpellPower>::new(30.0)).id();
let mage = app.world_mut().spawn(Base::<Life>::default()).id();
app.update();

// Cast: the spec carries the caster's spell power.
let fireball = app.world().snapshot(FireballHit.source(caster));
// The caster dies before it lands.
app.world_mut().despawn(caster);

app.world_mut().apply_effect(mage, fireball);
assert_eq!(app.world().get::<Base<Life>>(mage).unwrap().get(), 70.0);
```

In a behaviour, [`ActiveAbility::effect_spec`] makes the spec, and
`Snapshots` takes it. The projectile carries it until it lands:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Effect)] #[effect(name = "fireball.hit")] struct FireballHit;
# #[derive(Component, Default, Behaviour)] struct Fireball { target: Option<Entity> }
/// A fireball in flight, and what it hits with.
#[derive(Component)]
struct Projectile {
    target: Entity,
    hit: EffectSpec,
}

fn cast(
    casting: Query<(&ActiveAbility, &Fireball), Added<Fireball>>,
    snapshots: Snapshots,
    mut commands: Commands,
) {
    for (active, fireball) in &casting {
        let Some(target) = fireball.target else { continue };
        let hit = snapshots.take(active.effect_spec::<FireballHit>());
        commands.spawn(Projectile { target, hit });
    }
}

fn land(projectiles: Query<(Entity, &Projectile)>, mut commands: Commands) {
    for (projectile, flying) in &projectiles {
        // Once it reaches its target.
        let hit = flying.hit.clone().with(Causer(projectile));
        commands.entity(flying.target).apply_effect(hit);
        commands.entity(projectile).despawn();
    }
}
# App::new().add_systems(Update, (cast, land));
```

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
owns its owner tags. While it's inhibited, it carries neither, and its
time stands still. A periodic effect never carries modifiers: it executes
instead.

## Ticking

[`EffectPlugin`] ticks active effects in the first step of the [`Mettle`]
schedule, [`MettleSet::Effects`], each fixed step unless you run it
elsewhere. Each tick, for each effect:

1. If its `end_if` holds, it ends.
2. If its `ongoing` requirement stopped holding, it's inhibited. If it holds
   again, the effect counts again.
3. If it's counting, its period comes round as often as it's due, and its
   time runs down.
4. If its time ran out, it expires: it comes off, unless its [stacking](stacking.md)
   says otherwise.

Attributes fold the changes in the step after, [`MettleSet::Attributes`],
so they're published the same tick.

## Removing effects

Take effects off by their tags, or one by one, by despawning it:

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
            commands.entity(effect).despawn();
        }
    }
}
# #[derive(Component)] struct Drinking;
```

`remove_effects` goes on the actor, and [`EffectWorld`] has it on the
world; an active effect is taken off by despawning it, with commands or on
the world. Either way, the effect's tags and modifiers are taken back,
[`EffectRemoved`] is triggered, and its `on_remove` effects are chained.

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
| [`EffectBlocked`] | An effect didn't apply: `reason` is `Requirement`, `Effect` (another blocks it) or `StackLimit` |
| [`EffectExecuted`] | An effect changed bases: an instant one applied, or a period came round. `changes` says what it changed the target's bases by, `source_changes` its source's, and `routed` what it routed into each [route](effect-code.md#routes) |
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
effect would change, without changing it: a list of [`BaseChange`]s, each
named by number. An ability checks it can pay its cost this way. A tooltip
can show what a hit would do.

## Pitfalls

- **Removing it is despawning it.** Despawn an active effect, or take its
  `ActiveEffect` off, and it's told as removed and its `on_remove` chained,
  once commands are applied: at once on a `World`, which flushes as it
  despawns. Effects despawned with their target tell nothing: there's no
  one left to tell.
- **Commands wait.** `apply_effect` on commands applies when commands are
  applied, later in the frame. Read the result on the next system, or use
  `EffectWorld` in an exclusive system.
- **An actor with no attributes owns no tags.** Effects need the target's
  [`OwnedTags`]. Every `Base` brings them; on an actor without one, add
  `OwnedTags` yourself.

[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`EffectSpec`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectSpec.html
[`EffectWorld`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html
[`EffectWorld::preview_effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.preview_effect
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`EffectPlugin`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectPlugin.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Mettle.html
[`MettleSet::Effects`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.MettleSet.html#variant.Effects
[`MettleSet::Attributes`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/enum.MettleSet.html#variant.Attributes
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
[`EffectContext`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.EffectContext.html
[`AbilityRef`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.AbilityRef.html
[`Causer`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.Causer.html
[`SourceObject`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.SourceObject.html
[`Origin`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.Origin.html
[`Hit`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.Hit.html
[`ContextData`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/trait.ContextData.html
[`EffectWorld::snapshot`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.EffectWorld.html#tymethod.snapshot
[`Snapshots`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.Snapshots.html
[`ContextDataRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/context/struct.ContextDataRegistry.html
[`ActiveAbility::effect_spec`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.ActiveAbility.html#method.effect_spec
