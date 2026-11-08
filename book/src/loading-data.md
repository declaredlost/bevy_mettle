# Loading data

Tags, curves, effects and abilities are data. Mettle doesn't read files:
your game loads them however it likes (assets, a mod folder, a server's
download) and hands them over as one resource, [`MettleContent`]. This
chapter loads the mage's content from [RON](https://github.com/ron-rs/ron),
but any serde format works.

## The file

`MettleContent` is `Deserialize`. Every list is optional, and unknown
fields are errors, so a typo in a field name doesn't pass quietly.
Attributes are named in `snake_case` by default, so `Mana` is `"mana"`:

```ron
// content.ron
(
    tags: [(id: "cooldown"), (id: "cooldown.fireball"), (id: "state"), (id: "state.stunned")],
    effects: [
        (
            id: "fireball.cost",
            duration: Instant,
            modifiers: [(attribute: "mana", op: Flat, value: Value(-20))],
        ),
        (
            id: "fireball.cooldown",
            duration: For(Value(2)),
            grants: ["cooldown.fireball"],
        ),
    ],
    abilities: [
        (
            id: "fireball",
            behaviour: "cast_fireball",
            cost: "fireball.cost",
            cooldown: "fireball.cooldown",
            blocked_by: ["state.stunned"],
        ),
    ],
)
```

Content split over many files, one per item or one per mod, is merged
by your game: extend each list, then hand over the whole.

## Handing it over

Insert it as a resource. Mettle checks all of it at once, against the
attributes, behaviours and effect code the app added, before any part runs.
If it all holds, the [`TagTree`], [`CurveTable`], [`EffectRegistry`] and
[`AbilityRegistry`] are replaced, and [`ContentLoaded`] is triggered.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
# const CONTENT: &str = r#"(
#     tags: [(id: "cooldown"), (id: "cooldown.fireball"), (id: "state"), (id: "state.stunned")],
#     effects: [
#         (id: "fireball.cost", duration: Instant, modifiers: [(attribute: "mana", op: Flat, value: Value(-20))]),
#         (id: "fireball.cooldown", duration: For(Value(2)), grants: ["cooldown.fireball"]),
#     ],
#     abilities: [(id: "fireball", behaviour: "cast_fireball", cost: "fireball.cost", cooldown: "fireball.cooldown", blocked_by: ["state.stunned"])],
# )"#;
/// The mage, granted the fireball: its content is already loaded.
fn spawn_mage(mut commands: Commands) {
    commands
        .spawn(Base::<Mana>::default())
        .grant_ability("fireball", 1.0);
}

let content: MettleContent = ron::from_str(CONTENT).expect("content.ron");

# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins));
app.add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .insert_resource(content)
    .add_systems(Startup, spawn_mage);
# app.update();
# assert_eq!(app.world_mut().query::<&Abilities>().iter(app.world()).count(), 1);
```

Inserted as the app is built, content is loaded in `PreStartup`, so
`Startup` systems can grant and apply what it defines. Inserted later, by a
system once your files have loaded, it's loaded in the next `PreUpdate`.

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
    tags: vec![Tag { id: Id::new("cooldown") }],
    effects: vec![
        // A tag that isn't in the tree.
        Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        // A curve that isn't there.
        Effect::instant("fireball.cost").modifier(EffectModifier::new::<Mana>(
            Operation::Flat,
            Magnitude::scaled(-20.0, "fireball.cost"),
        )),
    ],
    ..default()
};
let errors = content.check(app.world()).unwrap_err();
assert_eq!(errors.0.len(), 2);
```

## Hot reloading

Change the resource and mettle loads it again, checked as a whole: active
effects take their new definitions, and granted abilities theirs. Replace
it as your files change:

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
  is built, before content is loaded.
- **Nothing until it's loaded.** Applying an effect or granting an ability
  before its content is loaded finds nothing, and warns.

[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`MettleContent::check`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html#method.check
[`ContentLoaded`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentLoaded.html
[`ContentRejected`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.ContentRejected.html
[`TagTree`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagTree.html
[`CurveTable`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/curve/struct.CurveTable.html
[`EffectRegistry`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectRegistry.html
[`AbilityRegistry`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilityRegistry.html
