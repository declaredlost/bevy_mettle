# Adding and tuning content

In Unreal's Gameplay Ability System a gameplay effect is a data asset. In
mettle it's code: effects, abilities and curves are types of yours that
define themselves, and the app adds them with `add_effect`, `add_ability`
and `add_curve`. Data does two things on top of that:

- **It adds content.** A mod's new spell, a server's seasonal event: tags,
  curves, effects and abilities under keys code doesn't define.
- **It tunes curves.** A curve in data with a code curve's key replaces it,
  as Unreal's curve tables tune scalable floats. Balancing a fireball's
  damage needs no rebuild.

Both go to mettle as one resource, [`MettleContent`]. Reading files isn't
mettle's business. Its data types are serde types, so your game loads them
however it likes: from any serde format, as Bevy assets, from a mod folder
or a server's download. A game that adds and tunes nothing hands over
nothing.

Mettle has no file format of its own yet. A designed one is
[issue #9](https://github.com/declaredlost/bevy_mettle/issues/9).

## What code defines

The mage's fireball and the curve its damage scales by are code:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 10.0), (10.0, 85.0)])]
struct FireballDamage;

#[derive(Effect)]
#[effect(flat::<Life>(Magnitude::scaled::<FireballDamage>(-1.0)))]
struct FireballHit;

#[derive(Ability)]
#[ability(behaviour::<CastFireball>, cost = Cost::of::<Mana>(20.0), cooldown = 2.0)]
struct Fireball;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Life>()
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .add_curve::<FireballDamage>()
    .add_effect::<FireballHit>()
    .add_ability::<Fireball>();
app.update();

// Loaded without any content.
let abilities = app.world().resource::<AbilityRegistry>();
assert!(abilities.id_of::<Fireball>().is_some());
```

[`MettleContentPlugin`], in [`MettlePlugins`], builds the [`TagRegistry`],
[`CurveRegistry`], [`EffectRegistry`] and [`AbilityRegistry`] from what code
defines in `PreStartup`, with or without content, so `Startup` systems can
grant and apply it. The fireball's cost and cooldown effects, and its
`cooldown.fireball` tag, are made from its definition there too.

## The content

[`MettleContent`] has a list for each kind of thing: `tags`, `curves`,
`effects` and `abilities`. Here is a mod's, built in code with the data
builders, as a test or a tool would. It adds a frost nova, run by a
behaviour the game's code adds, and granted by the game's staff while it's
equipped:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastNova;
# #[derive(Effect)] #[effect(infinite)] struct Staff;
fn frost_mod() -> MettleContent {
    MettleContent {
        tags: ["modded", "modded.frost"].map(TagKey::named).to_vec(),
        abilities: vec![
            AbilityData::new("frost_nova")
                .behaviour::<CastNova>()
                .tag("modded.frost")
                .cost(Cost::of::<Mana>(30.0))
                .cooldown(5.0)
                .granted_by::<Staff>(),
        ],
        ..Default::default()
    }
}
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Mana>()
#     .add_behaviour::<CastNova>()
#     .add_effect::<Staff>();
# frost_mod().check(app.world()).unwrap();
```

Content is named by key, as no type of the game's names it. It reaches the
game through what code defines: the nova is granted while the game's
`Staff` effect lasts. An ability in content declares its cost and cooldown
as one in code does, and mettle makes its effects the same way.

## Its serde shape

`MettleContent` is `Serialize` and `Deserialize`, and so is everything in
it. Whatever format your game reads, the shape is the same:

- **The content** is a struct of those four lists. Each can be left out,
  and is empty if it is. Unknown fields are errors, so a typo in a field's
  name doesn't pass quietly.
- **Each definition** is a struct with the fields its chapter lists:
  [`CurveData`](curves.md), [`EffectData`](effects.md) and
  [`AbilityData`](abilities.md). Fields that have a default can be left
  out. Unknown fields are errors here too.
- **Names are strings.** A tag is written as its name, `"modded.frost"`,
  and a set of tags as a list of names. So is what a definition names: an
  effect's `name`, the attributes, curves, effects, executions,
  calculations and routes it names, an ability's behaviour. Each of these is
  a key, a [`Key`] of its kind, such as `Key<AnyEffect>` for an effect.
  Code's types are named as their derives name them, in `snake_case` unless
  they say otherwise, so `Mana` is `"mana"` and `CastNova` is `"cast_nova"`.
- **Choices are enums**, written as the format writes an enum's variant:
  an effect's `duration` is `Instant`, `Infinite` or `For` with a
  [`Magnitude`], and a modifier's `op` is one of the [`Operation`]s.

Numbers never appear in data. Content names things; mettle numbers them as
it's checked (see [Names and numbers](concepts.md#names-and-numbers)).

## Tuning curves

A curve in content with the key of a curve code defines replaces it. The
code curve is the default, and data says otherwise:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(ScalingCurve)]
#[curve(keys = [(1.0, 10.0), (10.0, 85.0)])]
struct FireballDamage;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_curve::<FireballDamage>()
    // The balance pass: the fireball hits harder at level 1.
    .insert_resource(MettleContent {
        curves: vec![CurveData::new("fireball_damage").keys([(1.0, 15.0), (10.0, 85.0)])],
        ..default()
    });
app.update();

let curves = app.world().resource::<CurveRegistry>();
let damage = curves.id_of::<FireballDamage>().unwrap();
assert_eq!(curves.at(damage, 1.0), Some(15.0));
```

Every magnitude scaled by `FireballDamage` reads the tuned curve. Take the
curve out of the content and the code's comes back.

## Handing it over

Insert it as a resource. Mettle checks all of it at once, with what code
defines: the attributes, behaviours and effect code the app added, and its
tags, curves, effects and abilities. If it all holds, the registries are
replaced, and [`ContentLoaded`] is triggered.

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastNova;
# fn frost_mod() -> MettleContent {
#     MettleContent {
#         tags: ["modded", "modded.frost"].map(TagKey::named).to_vec(),
#         abilities: vec![AbilityData::new("frost_nova").behaviour::<CastNova>().tag("modded.frost").cost(Cost::of::<Mana>(30.0)).cooldown(5.0).granted_by::<Staff>()],
#         ..Default::default()
#     }
# }
#[derive(Effect)]
#[effect(infinite)]
struct Staff;

/// The mage, holding the staff: it grants what content says it grants.
fn spawn_mage(mut commands: Commands) {
    commands.spawn(Base::<Mana>::default()).apply_effect(Staff);
}

# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .insert_resource(TimeUpdateStrategy::FixedTimesteps(1));
app.add_attribute::<Mana>()
    .add_behaviour::<CastNova>()
    .add_effect::<Staff>()
    .insert_resource(frost_mod())
    .add_systems(Startup, spawn_mage);
# app.update();
# app.update();
# assert_eq!(app.world_mut().query::<&Abilities>().iter(app.world()).count(), 1);
```

Inserted as the app is built, content is loaded in `PreStartup`, with what
code defines. Inserted later, by a system once your files have loaded, it's
loaded in the next `PreUpdate`.

Content split over many files, one per item or one per mod, is merged by
your game: extend each list, then hand over the whole.

```rust
# use bevy_mettle::prelude::*;
/// Every file's content, as one.
fn merged(files: impl IntoIterator<Item = MettleContent>) -> MettleContent {
    let mut all = MettleContent::default();
    for file in files {
        all.tags.extend(file.tags);
        all.curves.extend(file.curves);
        all.effects.extend(file.effects);
        all.abilities.extend(file.abilities);
    }
    all
}
# let all = merged([
#     MettleContent { effects: vec![EffectData::new("amulet").infinite()], ..Default::default() },
#     MettleContent { effects: vec![EffectData::new("ring").infinite()], ..Default::default() },
# ]);
# assert_eq!(all.effects.len(), 2);
```

As a Bevy asset, the same goes: a loader of yours deserializes each file,
and a system inserts the merged content as the assets load or change.

### What code defines is code's

Content adds under new keys. An effect or ability code defines is code's:
content defining one with its key is rejected, so a mod can't redefine the
game's haste, and the game's code applies what its types say.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
#[effect(lasting = 5.0)]
struct Haste;

let mut app = App::new();
app.add_plugins(MettlePlugins).add_effect::<Haste>();

// A mod's take on the game's haste.
let modded = MettleContent {
    effects: vec![EffectData::new("haste")],
    ..Default::default()
};
let errors = modded.check(app.world()).unwrap_err();
assert_eq!(
    errors.to_string(),
    "content defines effect `haste`, which code defines"
);
```

Curves are the exception: tuning them is what content is for. Two
definitions of one key in content are an error too, so merging files, let a
later file's definition take an earlier one's place by name.

## Everything wrong, at once

Content that doesn't hold is rejected as a whole: each error is logged,
[`ContentRejected`] is triggered with all of them, and the content before
stays. Observe it to show whoever wrote the data:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn report(rejected: On<ContentRejected>) {
    for error in &rejected.errors.0 {
        eprintln!("{error}");
    }
}
# App::new().add_observer(report);
```

Tags and curves are checked first, then effects against them, then
abilities against the effects. An effect's errors show once the tags it
names hold, and an ability's once its effects do.

To ask without loading anything, such as a mod before it's enabled, call
[`MettleContent::check`] with the app's world:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# let mut app = App::new();
# app.add_plugins(MettlePlugins).add_attribute::<Life>();
let content = MettleContent {
    tags: vec![TagKey::named("modded")],
    effects: vec![
        // A tag that isn't defined.
        EffectData::new("frostbite").lasting(3.0).tag("modded.frost"),
        // An attribute that isn't added.
        EffectData::new("drain").modifier(EffectModifier::named(
            "spirit".into(),
            Operation::Flat,
            -5.0,
        )),
    ],
    ..default()
};
let errors = content.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);
```

## Reloading

Insert a changed `MettleContent`, or change the one there, and mettle loads
it again with what code defines, checked as a whole. What was there keeps
its number: every tag, curve, effect and ability already numbered keeps it,
and new ones are numbered after them, so whatever holds a number still
names the same thing. Active effects take their new definitions, and
granted abilities theirs.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
#[effect(infinite)]
struct Staff;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins)).add_effect::<Staff>();
app.update();
let staff = app.world().resource::<EffectRegistry>().id_of::<Staff>();

// A mod adds an amulet, whose name comes first.
app.insert_resource(MettleContent {
    effects: vec![EffectData::new("amulet").infinite()],
    ..default()
});
app.update();
let effects = app.world().resource::<EffectRegistry>();
assert_eq!(effects.id_of::<Staff>(), staff, "the staff keeps its number");
assert!(effects.id("amulet").unwrap() > staff.unwrap(), "the amulet's is new");
```

A fresh app numbers the same code and content in order of name. So a server
and its clients agree as long as each adds the same code and loads the same
content the same way: start from the same content, and reload the same
changes.

Replace it as your files change:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# fn reread() -> Option<MettleContent> { None }
fn reload(mut commands: Commands) {
    if let Some(content) = reread() {
        commands.insert_resource(content);
    }
}
```

A reload with a mistake in it is rejected, and the game keeps running on
the content before.

### Hot patching code

With the `hotpatching` feature, mettle follows Bevy's hot patching: run
the game under the Dioxus CLI, change an effect's, an ability's or a
curve's derive, and as the code is patched each type defines itself again
and the registries are rebuilt, in the next `PreUpdate`, as a reload is.
Numbers are kept the same way, and active effects and granted abilities
take their new definitions. See [Feature flags](features.md).

## Pitfalls

- **Code before content.** Content naming an attribute, a behaviour or
  effect code the app hasn't added is rejected. Add all of it as the app is
  built, before content is loaded.
- **New keys only.** Content defining an effect or ability with a key code
  defines is rejected. Content tunes curves; it doesn't redefine effects
  and abilities. See [What code defines is code's](#what-code-defines-is-codes).
- **Nothing until it's loaded.** Applying an effect or granting an ability
  content adds, before the content is loaded, finds nothing, and warns.
- **One reload, everywhere.** A server that reloads alone numbers new things
  its clients don't know. Reload both sides alike. Over lightyear, a client
  that numbers content otherwise is told: see
  [Checking content](lightyear.md#checking-content).

[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`MettleContent::check`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html#method.check
[`MettleContentPlugin`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContentPlugin.html
[`MettlePlugins`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettlePlugins.html
[`ContentLoaded`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentLoaded.html
[`ContentRejected`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentRejected.html
[`TagRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRegistry.html
[`CurveRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`Key`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Key.html
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
