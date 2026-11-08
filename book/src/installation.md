# Installation and features

Add `bevy_mettle` to your `Cargo.toml`:

```toml
[dependencies]
bevy_mettle = "0.1"
```

It needs Bevy 0.20 and Rust 1.99 or later, edition 2024.

Attributes, effects and abilities are on by default. Each part needs the
one before it: abilities pay with effects, and effects change attributes.

| Feature | Adds |
|---|---|
| `attribute` | Attributes and modifiers (default) |
| `effect` | Effects; turns on `attribute` (default) |
| `ability` | Abilities; turns on `effect` (default) |
| `lightyear` | Prediction over lightyear; turns on `ability` |

Gameplay tags, curves and conditions are always there.
[Feature flags](features.md) has the full list.

## Adding the plugins

Add [`MettlePlugins`]. It adds the plugin of every part you turned on:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins))
        .add_attribute::<Mana>()
        .run();
}
```

Attributes are kept in `FixedPreUpdate`, effects tick in `FixedUpdate`,
and abilities are kept in `PreUpdate`. To keep effects in another
schedule, set [`EffectPlugin::new`] as with Bevy's own plugin groups:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins.set(EffectPlugin::new(Update))))
        .run();
}
```

[`AttributePlugin::new`] does the same for attributes.

## Using one part alone

To use one part alone, depend on its crate, such as
`bevy_mettle_attribute`, and add its plugin. Each part's crate depends on
the parts before it, not after.

If you add the plugins one by one, also add [`MettleSystemsPlugin`] and
[`MettleContentPlugin`]. The first puts the parts in [`MettleSystems`], and
in order where they share a schedule; [How it fits together](concepts.md)
explains that order. The second loads [`MettleContent`]; [Loading
data](loading-data.md) shows how.

## Pitfalls

- **`add_attribute` before the plugins.** `add_attribute` adds
  `AttributePlugin` itself if it isn't there yet. `MettlePlugins` added
  after it adds it again, and Bevy panics. Add `MettlePlugins` first.
- **Time.** Effects run out by Bevy's time. `DefaultPlugins` and
  `MinimalPlugins` both include `TimePlugin`.
- **Lightyear.** With the `lightyear` feature, `MettlePlugins` also adds
  `MettleNetPlugin`, which needs lightyear's plugins. Add `MettlePlugins`
  after them. [Over lightyear](lightyear.md) shows how.

[`MettlePlugins`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettlePlugins.html
[`MettleSystems`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleSystems.html
[`MettleSystemsPlugin`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleSystemsPlugin.html
[`MettleContentPlugin`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContentPlugin.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`EffectPlugin::new`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/struct.EffectPlugin.html#method.new
[`AttributePlugin::new`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributePlugin.html#method.new
