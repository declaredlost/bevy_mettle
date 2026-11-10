# Loading data

Tags, curves, effects, abilities and grants are data. Reading files isn't
mettle's business. Its data types are serde types, so your game loads them
however it likes: from any serde format, as Bevy assets, from a mod folder or
a server's download, or built in code. Then it hands them over as one
resource, [`MettleContent`].

Mettle has no file format of its own yet. A designed one is
[issue #9](https://github.com/declaredlost/bevy_mettle/issues/9).

## The content

[`MettleContent`] has a list for each kind of thing: `tags`, `curves`,
`effects`, `abilities`, and `grants` (see [Grants and
triggers](grants-triggers.md)). Here is the mage's, built in code with the
data builders:

```rust
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
fn content() -> MettleContent {
    MettleContent {
        tags: ["cooldown", "cooldown.fireball", "state", "state.stunned"]
            .map(TagKey::named)
            .to_vec(),
        effects: vec![
            EffectData::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        ],
        abilities: vec![
            AbilityData::new("fireball", "cast_fireball")
                .cost("fireball.cost")
                .cooldown("fireball.cooldown")
                .blocked_by("state.stunned"),
        ],
        ..Default::default()
    }
}
# assert_eq!(content().effects.len(), 2);
```

Code with a type for something can name it by type instead,
`AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>())`, and a
typo is a compile error (see [Data and code](concepts.md#data-and-code)).

## Its serde shape

`MettleContent` is `Serialize` and `Deserialize`, and so is everything in
it. Whatever format your game reads, the shape is the same:

- **The content** is a struct of those five lists. Each can be left out,
  and is empty if it is. Unknown fields are errors, so a typo in a field's
  name doesn't pass quietly.
- **Each definition** is a struct with the fields its chapter lists:
  [`CurveData`](curves.md), [`EffectData`](effects.md#fields),
  [`AbilityData`](abilities.md#the-fields) and [`Grant`](grants-triggers.md).
  Fields that have a default can be left out. Unknown fields are errors here
  too.
- **Names are strings.** A tag is written as its name,
  `"cooldown.fireball"`, and a set of tags as a list of names. So is what a
  definition names: an effect's `name`, the attributes, curves, effects,
  executions, calculations and routes it names, an ability's behaviour.
  Each of these is a key, a [`Key`] of its kind, such as `Key<AnyEffect>`
  for an effect.
  Attributes are named as their types are, in `snake_case` unless they say
  otherwise, so `Mana` is `"mana"`.
- **Choices are enums**, written as the format writes an enum's variant:
  an effect's `duration` is `Instant`, `Infinite` or `For` with a
  [`Magnitude`], and a modifier's `op` is one of the [`Operation`]s.

Numbers never appear in data. Content names things; mettle numbers them as
it's checked (see [Names and numbers](concepts.md#names-and-numbers)).

## Handing it over

Insert it as a resource. Mettle checks all of it at once, against the
attributes, behaviours and effect code the app added and the tags, effects
and abilities code names, before any part runs. If it all holds, the
[`TagRegistry`], [`CurveRegistry`], [`EffectRegistry`] and
[`AbilityRegistry`] are replaced, and [`ContentLoaded`] is triggered.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
# fn content() -> MettleContent {
#     MettleContent {
#         tags: ["cooldown", "cooldown.fireball", "state", "state.stunned"].map(TagKey::named).to_vec(),
#         effects: vec![
#             EffectData::instant("fireball.cost").modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
#             EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
#         ],
#         abilities: vec![AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>()).cost("fireball.cost").cooldown("fireball.cooldown").blocked_by("state.stunned")],
#         ..Default::default()
#     }
# }
#[derive(Ability)]
struct Fireball;

/// The mage, granted the fireball: its content is already loaded.
fn spawn_mage(mut commands: Commands) {
    commands.spawn(Base::<Mana>::default()).grant_ability(Fireball);
}

# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins));
app.add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .add_ability::<Fireball>()
    .insert_resource(content())
    .add_systems(Startup, spawn_mage);
# app.update();
# assert_eq!(app.world_mut().query::<&Abilities>().iter(app.world()).count(), 1);
```

Inserted as the app is built, content is loaded in `PreStartup`, so
`Startup` systems can grant and apply what it defines. Inserted later, by a
system once your files have loaded, it's loaded in the next `PreUpdate`.

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
        all.grants.extend(file.grants);
    }
    all
}
# let all = merged([
#     MettleContent { effects: vec![EffectData::infinite("staff")], ..Default::default() },
#     MettleContent { effects: vec![EffectData::infinite("ring")], ..Default::default() },
# ]);
# assert_eq!(all.effects.len(), 2);
```

As a Bevy asset, the same goes: a loader of yours deserializes each file,
and a system inserts the merged content as the assets load or change.

### What code names stays

An effect or ability code names (`add_effect`, `add_ability`) is one code
counts on: something applies or grants it. Content has to define it, or it's
rejected: a mod can't take it away. A mod that wants it gone redefines it
doing nothing instead, and code goes on applying it, to no effect:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
struct Haste;

let mut app = App::new();
app.add_plugins(MettlePlugins).add_effect::<Haste>();

// The game's haste, and a mod's take on it: haste, doing nothing.
let game = MettleContent {
    effects: vec![EffectData::lasting("haste", 5.0)],
    ..Default::default()
};
let modded = MettleContent {
    effects: vec![EffectData::instant("haste")],
    ..Default::default()
};
assert!(game.check(app.world()).is_ok());
assert!(modded.check(app.world()).is_ok());

// Without haste at all, it's rejected.
let errors = MettleContent::default().check(app.world()).unwrap_err();
assert_eq!(
    errors.to_string(),
    "code names effect `haste`, which content doesn't define"
);
```

Merging files, let a later file's definition take an earlier one's place by
name, so a mod's `haste` replaces the game's rather than clashing with it.

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
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# let mut app = App::new();
# app.add_plugins(MettlePlugins).add_attribute::<Mana>();
let content = MettleContent {
    tags: vec![TagKey::named("cooldown")],
    effects: vec![
        // A tag that isn't defined.
        EffectData::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        // A curve that isn't there.
        EffectData::instant("fireball.cost").modifier(EffectModifier::new::<Mana>(
            Operation::Flat,
            Magnitude::scaled(-20.0, "fireball.cost"),
        )),
    ],
    ..default()
};
let errors = content.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);
```

## Reloading

Insert a changed `MettleContent`, or change the one there, and mettle loads
it again, checked as a whole. What was there keeps its number: every tag,
curve, effect and ability already numbered keeps it, and new ones are
numbered after them, so whatever holds a number still names the same thing.
Active effects take their new definitions, and granted abilities theirs.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Effect)]
struct Staff;

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_effect::<Staff>()
    .insert_resource(MettleContent {
        effects: vec![EffectData::infinite("staff")],
        ..default()
    });
app.update();
let staff = app.world().resource::<EffectRegistry>().id_of::<Staff>();

// A mod adds an amulet, whose name comes first.
app.insert_resource(MettleContent {
    effects: vec![EffectData::infinite("amulet"), EffectData::infinite("staff")],
    ..default()
});
app.update();
let effects = app.world().resource::<EffectRegistry>();
assert_eq!(effects.id_of::<Staff>(), staff, "the staff keeps its number");
assert!(effects.id("amulet").unwrap() > staff.unwrap(), "the amulet's is new");
```

A fresh app numbers the same content in order of name. So a server and its
clients agree as long as each loads the same content the same way: start
from the same content, and reload the same changes.

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

## Pitfalls

- **Code before content.** Content naming an attribute, a behaviour or
  effect code the app hasn't added yet is rejected. Add them all as the app
  is built, before content is loaded. The same goes for the effects and
  abilities code names with `add_effect` and `add_ability`: content that
  doesn't define one is rejected.
- **What code names, content keeps.** A mod can't delete an effect or ability
  code adds; it redefines it doing nothing. See
  [What code names stays](#what-code-names-stays).
- **Nothing until it's loaded.** Applying an effect or granting an ability
  before its content is loaded finds nothing, and warns.
- **One reload, everywhere.** A server that reloads alone numbers new things
  its clients don't know. Reload both sides alike. Over lightyear, a client
  that numbers content otherwise is told: see
  [Checking content](lightyear.md#checking-content).

[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`MettleContent::check`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html#method.check
[`ContentLoaded`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentLoaded.html
[`ContentRejected`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentRejected.html
[`TagRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRegistry.html
[`CurveRegistry`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveRegistry.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
[`Key`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Key.html
[`Magnitude`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/data/enum.Magnitude.html
[`Operation`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/modifier/enum.Operation.html
