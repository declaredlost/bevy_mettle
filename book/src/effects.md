# Defining effects

An effect is what changes an actor: damage, a buff, a stun. It says what it
changes, for how long, and what tags its owner holds while it lasts. It's
Unreal's `GameplayEffect`. In Unreal that's a data asset; in mettle it's
code: a type that defines itself.

## The mage's effects

The fire mage from earlier chapters needs two effects:

- `burning`, fire damage every half second for four seconds.
- `staff`, a staff the mage holds for as long as it's equipped.

Each is a unit struct deriving [`Effect`]. Each key on `#[effect(..)]` is
the [`EffectData`] builder method of the same name: `lasting = 4.0` is
`.lasting(4.0)`, `execute_on_apply` is `.execute_on_apply()`, and
`flat::<Life>(-5.0)` is `.flat::<Life>(-5.0)`. A key that isn't a method
doesn't compile.

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# #[derive(Attribute)]
# #[attribute(base = 100.0, min = 0.0)]
# struct Life;
# #[derive(Tag)] #[tag(name = "effect")] struct Effects;
# #[derive(Tag)] #[tag(parent = Effects)] struct Affliction;
# #[derive(Tag)] #[tag(parent = Affliction, name = "burning")] struct Burns;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State, name = "burning")] struct OnFire;
# #[derive(Tag)] #[tag(parent = State)] struct Wet;
# #[derive(Tag)] struct Immune;
# #[derive(Tag)] #[tag(parent = Immune, name = "fire")] struct ImmuneToFire;
/// How hard a burn hits: set by the code applying it.
#[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
#[reflect(ContextData)]
struct Damage {
    amount: f32,
}

/// Fire damage every half second for four seconds, and once as it's applied.
#[derive(Effect)]
#[effect(lasting = 4.0, period = 0.5, execute_on_apply)]
#[effect(tag = Burns, owner_tag = OnFire)]
#[effect(flat::<Life>(Magnitude::context::<Damage>("amount").times(-1.0)))]
#[effect(require = !Condition::owner_has(ImmuneToFire))]
#[effect(ongoing = !Condition::owner_has(Wet))]
struct Burning;                    // "burning"

/// Held for as long as it's equipped.
#[derive(Effect)]
#[effect(infinite)]
struct Staff;                      // "staff"

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .add_tag::<Burns>()
    .add_tag::<OnFire>()
    .add_tag::<Wet>()
    .add_tag::<ImmuneToFire>()
    .register_type::<Damage>()
    .add_effects::<(Burning, Staff)>();
# // Context data is numbered as the app finishes, which `run` does.
# app.finish();
# app.cleanup();
app.update();

let registry = app.world().resource::<EffectRegistry>();
assert!(registry.id_of::<Burning>().is_some());
```

Add an effect with `add_effect::<Burning>()`, or several at once with
`add_effects::<(Burning, Staff)>()`, as the app is built. The type is how
code applies it: `apply_effect(Burning)`, as [Applying
effects](applying-effects.md) shows. A misspelled type doesn't compile.

An effect's key is its type's name in `snake_case`, unless `name` says
otherwise: `#[effect(name = "fireball.hit")]`. Its keys can be spread over
several `#[effect(..)]` attributes, as `Burning`'s are. The struct is a
marker, with no fields: what changes from one application to the next, such
as how hard it hits, goes on the spec's [context](applying-effects.md#context),
for magnitudes to read with `Magnitude::context`.

The tags it names are added with `add_tag`, as [Gameplay
tags](tags.md#tags-in-code) shows. The staff does nothing yet. [Grants and
triggers](grants-triggers.md) gives it an ability to grant.

An ability's cost and cooldown are effects too, but mettle makes them from
the ability: see [Defining abilities](abilities.md).

### Long definitions

A definition too long for its keys goes in a function, named by `define`.
It's handed an instant effect called by the type's key, and fills it in:

```rust
# use bevy::prelude::*;
# use bevy::ecs::entity::MapEntities;
# use bevy_mettle::prelude::*;
# use serde::{Deserialize, Serialize};
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Tag)] #[tag(name = "effect")] struct Effects;
# #[derive(Tag)] #[tag(parent = Effects)] struct Affliction;
# #[derive(Tag)] #[tag(parent = Affliction, name = "burning")] struct Burns;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State, name = "burning")] struct OnFire;
# #[derive(Tag)] #[tag(parent = State)] struct Wet;
# #[derive(Tag)] struct Immune;
# #[derive(Tag)] #[tag(parent = Immune, name = "fire")] struct ImmuneToFire;
# #[derive(Reflect, Clone, PartialEq, Serialize, Deserialize, MapEntities)]
# #[reflect(ContextData)] struct Damage { amount: f32 }
#[derive(Effect)]
#[effect(define = burning)]
struct Burning;

fn burning(effect: EffectData) -> EffectData {
    effect
        .lasting(4.0)
        .period(0.5)
        .execute_on_apply()
        .tag(Burns)
        .owner_tag(OnFire)
        .flat::<Life>(Magnitude::context::<Damage>("amount").times(-1.0))
        .require(!Condition::owner_has(ImmuneToFire))
        .ongoing(!Condition::owner_has(Wet))
}
# let burning = <Burning as Effect>::define(EffectData::new(Key::of::<Burning>()));
# assert_eq!(burning.name.as_str(), "burning");
# assert!(burning.lasts());
```

`define` defines all of it, so it takes no other keys but `name`.

## What an effect says

Every key is a builder method, and every method sets one of
[`EffectData`]'s fields. Everything is empty unless the effect says, and an
effect is instant unless it lasts.

| Key | Field | What it does |
|---|---|---|
| `name = ".."` | `name` | Its key: its type's name in `snake_case`, unless it says |
| `lasting = s`, `infinite` | `duration` | `For(s)` seconds, or `Infinite`; `Instant` unless it says |
| `tag = t` | `tags` | What it is: what ending, blocking and requirements match on |
| `period = s` | `period` | Executes every so often while it lasts, instead of its modifiers counting all along |
| `execute_on_apply` | `execute_on_apply` | A periodic effect executes as it's applied too |
| `flat::<A>(v)`, `route::<R>(v)`, `modifier = m` | `modifiers` | What it changes: modifiers while it lasts, or the base as it executes |
| `execution::<E>` | `executions` | Formulas in code, run as it executes: see [Effects in code](effect-code.md) |
| `owner_tag = t` | `owner_tags` | Tags the target, its owner, holds while it counts |
| `cue = t` | `cues` | Cues held on the target while it counts, and played each time it executes: see [Cues](cues.md) |
| `require = c` | `require` | What has to hold for it to apply at all |
| `ongoing = c` | `ongoing` | What has to hold for it to count; while it doesn't, it's inhibited |
| `end_if = c` | `end_if` | What ends it as soon as it holds |
| `ends = t` | `ends` | Active effects with any of these tags, ended as it applies |
| `blocks = t` | `blocks` | Effects with any of these tags can't apply while it counts |
| `stacking = s` | `stacking` | How applying it again stacks: see [Stacking](stacking.md) |
| `on_apply::<E>`, `on_remove::<E>`, `on_expire::<E>` | `on_apply`, `on_remove`, `on_expire` | Effects applied to the target as it applies, comes off, or runs out of time |

Keys that add to a list, such as `tag` and `flat`, can be given more than
once.

## Durations

- **Instant** executes once and is gone. Its modifiers change the target's
  bases: a potion's 30 life is healed for good.
- **`infinite`** lasts until something takes it off: gear, an aura, a curse
  that has to be cleansed.
- **`lasting = seconds`** lasts that long. Its time runs down as effects
  tick.

A lasting effect's modifiers count as modifiers of the target while it
counts, and stop counting when it goes. The base is left alone, so a
two-second buff can't leave anything behind.

An instant effect can't have anything that needs it to last: a period,
owner tags, blocked effects, an ongoing or end condition, stacking, or
effects on removal or expiry. Mettle rejects it as it checks the effects.

## Periods

A period turns a lasting effect into one that executes every so often, like
an instant effect would, for as long as it lasts. `Burning` executes every
half second, and once more as it's applied, since it says
`execute_on_apply`.

A periodic effect's modifiers never count as modifiers. Each execution
changes the base instead, so burning's damage stays taken when it ends.

If a tick is longer than the period, the effect executes as many times as
periods came round. A period has to be more than 0.

## Modifiers

An [`EffectModifier`] names an attribute, an [`Operation`] and a
[`Magnitude`]. `flat::<Life>(v)` is the common one, a `Flat` modifier that
counts always; `modifier = EffectModifier::new::<A>(op, v)` takes any
operation, a condition and a `per`. In a lasting effect, it's a modifier like
any other, folded as [Modifiers](modifiers.md) explains. As an effect
executes, it changes the base instead:

| Op | What executing does to the base |
|---|---|
| `Flat`, `Unscaled` | Adds the value |
| `Additive`, `Multiplicative` | Scales by `1 + value` |
| `Divide` | Divides by `1 + value` |
| `Base`, `Override` | Sets it to the value |

The base is then rounded and kept within the attribute's limits, so burning
can't take life below 0.

A modifier can have a condition, with `.condition(..)`. In a lasting effect,
it's weighed on each read, as an item's would be:

- `ReadTagged` matches the read's tags: +20% damage with fire hits.
- `OwnerHas` matches the target's owned tags.
- `TargetHas` matches the owned tags of the read's target.
- `SourceHas` matches what the effect's source owned when it applied.

As an effect executes, `ReadTagged` matches the effect's own tags, and
`OwnerHas` and `TargetHas` match the target's. A condition that holds in part
scales the amount. A `Base` or `Override` counts only when it holds in full.

`.per::<A>(every)` makes a modifier count once for each whole `every` of the
target's `A`: +1 damage per 10 spell power.

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] struct Damage;
# #[derive(Attribute)] struct SpellPower;
# #[derive(Tag)] struct Undead;
/// Half again as much damage against the undead, and 1 more per 10 spell
/// power, for ten seconds.
#[derive(Effect)]
#[effect(lasting = 10.0)]
#[effect(modifier = EffectModifier::new::<Damage>(Operation::Additive, 0.5)
    .condition(Condition::target_has(Undead)))]
#[effect(modifier = EffectModifier::new::<Damage>(Operation::Flat, 1.0).per::<SpellPower>(10.0))]
struct Consecrated;
```

A modifier can route into a [route](effect-code.md#routes) instead, with
`route::<IncomingDamage>(40.0)`: an amount no actor holds, routed into
attributes they do as the effect executes. A route takes flat amounts only,
from an effect that executes: instant, or periodic. Mettle rejects any other.

An effect names attributes, routes, executions and calculations by type.
Checking the effects resolves each to its number, which is what the effect
runs by: see [Names and numbers](concepts.md#names-and-numbers).

## Magnitudes

A [`Magnitude`] is how much: a modifier's value, a duration, or a period.
It's a small expression. Its terms are:

| Term | How much | In code |
|---|---|---|
| `Value(f32)` | That number | A plain `f32` converts to it |
| `Level` | The spec's level | `Magnitude::Level` |
| `Context { data, field }` | An `f32` field of context data the spec carries | [`Magnitude::context::<Damage>("amount")`][Magnitude::context] |
| `Attribute(read)` | An attribute of the source's or the target's, an [`AttributeRead`]: see [Captures](#captures) | [`Magnitude::source`], [`Magnitude::target`] |
| `Custom(name)` | A calculation in code: see [Effects in code](effect-code.md) | [`Magnitude::calculation::<BurnTime>()`][Magnitude::calculation] |
| `Curve { curve, at }` | The [curve](curves.md)'s value at `at`, any magnitude: the spec's level unless it says | [`Magnitude::curve`], [`Magnitude::curve_at`] |

And it combines them, with [`times`], [`plus`] and [`clamp`]:

| Combination | How much |
|---|---|
| `Sum(terms)` | The terms added up |
| `Product(terms)` | The terms multiplied together |
| `Clamp { value, min, max }` | `value`, kept within `min` and `max`, either of them left out |

`Context` is Unreal's `SetByCaller`. The applying code knows something the
effect doesn't, such as how hard the fireball hit, and puts it on the spec
as context data: `Burning.with(Damage { amount: 5.0 })`. The field has to
be an `f32`. A spec without the data gets 0, with a warning.

### Unreal's magnitudes

Unreal has four kinds of magnitude, each with its own extras: a coefficient
here, a curve there. In mettle, each is a composition of the terms:

| Unreal | Mettle |
|---|---|
| Scalable float | `Product[Value, Curve]`: `Magnitude::scaled::<FireballDamage>(10.0)` |
| Attribute based, `coef × (attr + pre) + post` | `Sum` and `Product` of an `Attribute`, each part any magnitude, a curve too |
| Attribute based, by an attribute curve | `Curve { at: Attribute }` |
| Custom calculation class, its coefficient | `Product[Custom, Value]` |
| Set by caller | `Context`: a field of the spec's context data |
| Source and target tag filters | The read's `tagged` tags |

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
# #[derive(Attribute)] struct Armour;
# #[derive(Tag)] struct Damage;
# #[derive(Tag)] #[tag(parent = Damage)] struct Fire;
#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 1.0), (10.0, 4.0)])]
struct FireballDamage;

#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 0.1), (10.0, 0.5)])]
struct WardScaling;

#[derive(ScalingCurve)]
#[curve(keys = [(0.0, 1.0), (100.0, 0.5)])]
struct ArmourFalloff;

#[derive(Calculation)]
struct BurnTime;

// A scalable float: 10 times the curve at the spec's level.
assert_eq!(
    Magnitude::scaled::<FireballDamage>(10.0),
    Magnitude::Product(vec![10.0.into(), Magnitude::curve::<FireballDamage>()]),
);

// Attribute based: half of (the caster's bonus spell power plus 4), plus 10.
let ward = Magnitude::source::<SpellPower>().bonus().plus(4.0).times(0.5).plus(10.0);

// Its coefficient scaling with level, as Unreal's scalable coefficient.
let scaling = Magnitude::source::<SpellPower>().times(Magnitude::curve::<WardScaling>());

// An attribute curve: the target's armour, looked up on a falloff curve.
let falloff = Magnitude::curve_at::<ArmourFalloff>(Magnitude::target::<Armour>());

// A calculation, times 1.5.
let burn = Magnitude::calculation::<BurnTime>().times(1.5);

// Read in a tag: the target's armour as fire hits see it, at least 0.
let armour = Magnitude::from(Magnitude::target::<Armour>().tagged(Fire))
    .clamp(Some(0.0), None);
```

Curves are added with `add_curve`, as [Curves](curves.md) shows, and data can
tune them by key.

It's deliberately limited to sums, products and clamps: no division, no
conditions, no variables. Anything more is a calculation, in code.

Checking an effect turns each magnitude into a [`MagnitudeDef`]: what it
names resolved to numbers, so no string reaches the game as it runs, and its
constants folded, so `Product[2, 3]` is 6.

## Captures

An `Attribute` magnitude reads an [`AttributeRead`]. Unreal calls it an
attribute capture:

| Field | What it is |
|---|---|
| `attribute` | The attribute |
| `from` | Whose: `Source` unless it says, or `Target` |
| `value` | Which of its values: `Current` unless it says, `Base`, or `Bonus` (its current value less its base) |
| `snapshot` | Captured and kept, unless it says `false`: then it's read live |
| `tags` | What it's read in: which conditional [modifiers](modifiers.md) count |

[`Magnitude::source`] and [`Magnitude::target`] start one, and `.base()`,
`.bonus()`, `.live()` and `.tagged(tag)` set the rest; it's a magnitude
wherever one's taken.

A capture is read as the hit it's part of is: in the effect's tags and its
own, with the effect's target and source and what each owns, as any [read in
context](reading.md) is. So a caster's +20% damage with fire hits counts for
a fireball tagged `damage.fire`, and +10% against burning targets counts on
a burning target, with nothing more said. `.tagged(tag)` reads in more. With
no source, a source's attribute is 0.

### Snapshot or live

A capture is a snapshot unless it says, as Unreal's `bSnapshot`. A snapshot
is taken at the earliest moment, and kept:

- **The source's** as the spec is made, if the code making it [takes its
  snapshot](applying-effects.md#snapshots); if not, as it's applied.
- **The target's** as it's applied.

New stacks and periods use what was captured, not the caster's spell power
now.

`.live()` reads it as it is. An effect that executes reads it each time it
does. A lasting effect follows it: when the attribute changes, the effect's
modifiers are worked out again in the same step, before values are
published. Only the effects reading the actor that changed are, and nothing
polls. An aura that grows with its caster reads the caster's spell power
live:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
#[derive(Attribute)]
struct SpellPower;

#[derive(Attribute)]
struct Armour;

/// Follows the caster's spell power.
#[derive(Effect)]
#[effect(infinite, flat::<Armour>(Magnitude::source::<SpellPower>().live().times(0.5)))]
struct Aura;

/// Keeps what it was as it applied.
#[derive(Effect)]
#[effect(infinite, flat::<Armour>(Magnitude::source::<SpellPower>().times(0.5)))]
struct Blessing;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   // One fixed step, where mettle runs, each update.
#   .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
    .add_attribute::<SpellPower>()
    .add_attribute::<Armour>()
    .add_effects::<(Aura, Blessing)>();
app.update();

let caster = app.world_mut().spawn(Base::<SpellPower>::new(20.0)).id();
let (warded, blessed) = (
    app.world_mut().spawn(Base::<Armour>::default()).id(),
    app.world_mut().spawn(Base::<Armour>::default()).id(),
);
app.update();
app.world_mut().apply_effect(warded, Aura.source(caster));
app.world_mut().apply_effect(blessed, Blessing.source(caster));
let armour = |app: &App, actor| app.world().get::<Current<Armour>>(actor).unwrap().get();
app.update();
assert_eq!((armour(&app, warded), armour(&app, blessed)), (10.0, 10.0));

// The caster grows stronger: the aura follows, the blessing doesn't.
app.world_mut().get_mut::<Base<SpellPower>>(caster).unwrap().set(40.0);
app.update();
assert_eq!((armour(&app, warded), armour(&app, blessed)), (20.0, 10.0));
```

An effect can't follow itself. A lasting effect reading live what it changes
of the target's would change it, read the change, and change it again, so
mettle rejects it: "a live read of the target's `armour`, which it changes:
it would follow itself".

## Requirements

Three conditions decide whether an effect applies, counts, and stays. Each
is a [condition](conditions.md), weighed against the effect's own tags (as
`ReadTagged`), the target's owned tags (as `OwnerHas` and `TargetHas`), and
what the source owned as it applied (as `SourceHas`). It has to hold in full.
Facts are never set here.

- **`require`** is checked once, as it's applied. If it doesn't hold, the
  effect doesn't apply, and [`EffectBlocked`] says why. Burning doesn't apply
  to a mage who owns `immune.fire`.
- **`ongoing`** is checked every tick. While it doesn't hold, the effect is
  inhibited: its modifiers stop counting, its tags are taken back, and its
  time stands still. Burning is inhibited while the mage is wet, and picks
  up where it left off when the mage dries.
- **`end_if`** is checked every tick. As soon as it holds, the effect
  ends.

## Ending and blocking

`ends` ends active effects on the target as the effect applies: a potion
with `ends = Affliction` cleanses burning and poison. It's done before the
new effect lands, so it never ends itself.

`blocks` keeps effects off while it counts: Unreal's immunity. A fire ward
with `blocks = Burns` blocks burning, with [`EffectBlocked`]'s reason
`Effect`. An inhibited ward protects no one.

Tags match by their tree: ending `effect.affliction` ends everything tagged
under it.

Abilities have the same pair under the same names: an ability `ends` the
active abilities it matches as it starts, and `blocks` others while it's
active. Each has `end_if` too.

## Chains

An effect can apply other effects to its target, each named by type:

- `on_apply::<E>` as it applies, and again as it gains a stack.
- `on_remove::<E>` as it comes off, however that happens.
- `on_expire::<E>` as it runs out of time, after `on_remove`.

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
/// What a burn leaves as it burns out.
#[derive(Effect)]
#[effect(lasting = 3.0, flat::<Life>(-10.0))]
struct Scorched;

#[derive(Effect)]
#[effect(lasting = 4.0, period = 0.5, flat::<Life>(-2.0), on_expire::<Scorched>)]
struct Smouldering;
```

A chained effect is applied with the same spec: the same level, context,
source and activation. A chain more than 16 effects deep is taken for a
loop, and stopped with a warning.

What a chain can't say, such as spawning flames as a burn starts, goes in an
observer: see [What's told](applying-effects.md#whats-told).

## Checking them

Mettle checks every effect code defines, and every effect content adds, as
the app starts, in `PreStartup`. It checks them against the [tag
registry](tags.md#the-tag-registry), the [curves](curves.md), the attributes
the app added, and the [effect code](effect-code.md), and turns them into the
[`EffectRegistry`]. It reports every error at once:

- effects defined twice, and content defining an effect code defines;
- tags, curves, attributes, context data, effects, executions,
  calculations and routes that aren't added;
- numbers that can't be: not finite, a duration or period of 0 or less, a
  `per` of 0 or less, a stacking limit of 0;
- magnitudes that can't be: a sum or product of nothing, a clamp whose
  least is above its most;
- a lasting effect reading live what it changes of the target's;
- instant effects with parts that need them to last;
- executions on a lasting effect with no period, which never executes.

What doesn't check out is rejected: each error is logged,
[`ContentRejected`] is triggered with them, and what was loaded before
stays. To ask without loading anything, call [`MettleContent::check`] with
the content to add, or none:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use bevy_mettle::core::Errors;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State, name = "burning")] struct OnFire;
#[derive(Effect)]
#[effect(lasting = 4.0, owner_tag = OnFire)]
struct Burning;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .add_effect::<Burning>();

// A tag the effect names that the app didn't add is caught before anything runs.
let errors = MettleContent::default().check(app.world()).unwrap_err();
assert_eq!(errors.to_string(), "there's no tag called `state.burning` (in `burning`)");

app.add_tag::<OnFire>();
MettleContent::default().check(app.world())?;
# Ok::<(), Errors>(())
```

Mettle doesn't load files. Data only adds effects, by keys code doesn't
define, such as a mod's: see [Adding and tuning content](content.md).

## Numbers

As effects are checked, each is numbered: an [`EffectId`], given in order of
name, as Bevy gives a component its `ComponentId`. Active effects, chains,
events and the network all go by the number. The name is for data, debug
output and what a client shows. The registry goes between them:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Effect)] #[effect(lasting = 4.0)] struct Burning;
# #[derive(Effect)] #[effect(infinite)] struct Staff;
let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_effects::<(Burning, Staff)>();
app.update();

let registry = app.world().resource::<EffectRegistry>();
let burning = registry.id("burning").unwrap();
assert_eq!(registry.id_of::<Burning>(), Some(burning));
assert_eq!(registry.name(burning), Some("burning"));
assert!(registry.get(burning).is_some());
assert_eq!(registry.iter().count(), 2);
```

[`EffectRegistry`] reads as every registry does: `get(id)` gives the
checked effect, `id("name")` and `id_of::<E>()` its number, `name(id)` its
name, and `iter()` every one. [`EffectRegistry::rebuild`] is the check on
its own, against an [`EffectScope`] of the tag, curve, attribute,
execution, calculation, route and context data registries: it defines each
effect type again, adds content's, and keeps the numbers effects had.

Numbered in order of name, the same effects are numbered the same wherever
they're checked: a server and its clients agree on every number without
sending names.

## Rebuilding

Mettle checks and loads the effects again, as a whole, as
[`MettleContent`] changes, and, with the `hotpatching` feature, as code is
hot patched: each type defines itself again, so a changed `#[effect(..)]`
takes hold without a restart. Effects keep the numbers they had, and new
ones are numbered after them, so what holds a number still names the same
effect.

Every active effect takes its new definition on the next tick. Its
modifiers and granted tags are worked out again, and its captures are taken
again: the source's from the spec's snapshot, if it has one. Its time left
is kept. An active effect whose definition is gone comes off, as if removed.

## Pitfalls

- **Damage is negative.** A `Flat` modifier adds. Burning reads a positive
  amount from its context and multiplies it by -1.
- **Add effects as the app is built.** Effects are checked as the app
  starts. One added after that isn't there until they're checked again.
- **Add the tags they name.** An effect naming a tag type the app didn't
  add with `add_tag` is rejected, as is one naming context data not
  registered with `register_type`.
- **A periodic effect's modifiers aren't buffs.** Each period changes the
  base, and the change stays. For a buff that comes and goes, leave out the
  period.

[`EffectRegistry::rebuild`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html#method.rebuild
[`EffectScope`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectScope.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`MettleContent::check`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html#method.check
[`ContentRejected`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentRejected.html
[`EffectData`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.EffectData.html
[`Effect`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/trait.Effect.html
[`EffectId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.EffectId.html
[`EffectModifier`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.EffectModifier.html
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[Magnitude::context]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.context
[Magnitude::calculation]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.calculation
[`Magnitude::source`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.source
[`Magnitude::target`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.target
[`Magnitude::curve`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.curve
[`Magnitude::curve_at`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.curve_at
[`times`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.times
[`plus`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.plus
[`clamp`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html#method.clamp
[`MagnitudeDef`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.MagnitudeDef.html
[`AttributeRead`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/struct.AttributeRead.html
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
[`EffectBlocked`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/events/struct.EffectBlocked.html
