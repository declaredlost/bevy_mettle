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

## Where mettle runs

Mettle's gameplay runs in its own schedule, [`Mettle`], in four steps, its
[`MettleSet`]s, one after another:

1. **`Effects`**: active effects ticked. Requirements weighed, periods come
   round, time run out.
2. **`Attributes`**: bases synced, modifiers folded, values worked out.
3. **`Settle`**: effects reading another actor's attribute live follow it,
   and what that changed is folded again, until nothing moves.
4. **`Publish`**: `Current<A>` written, `AttributeChanged` told.

[`MettleCorePlugin`] runs `Mettle` in `FixedPostUpdate`, the way Bevy's
`Main` runs its schedules. It's the fixed step for prediction and rollback:
a tick is the unit a client predicts and a server confirms, and a rollback
runs the ticks again. What your systems do in a tick's `FixedUpdate` is
ticked, folded and published before that tick ends, so a replayed tick
comes out the same.

Two things run outside it. Abilities handle network messages in `PreUpdate`
([`AbilitySystems`]), where transports hand them over. Cues play in
`PostUpdate`.

### Ordering your systems

`Mettle` is run by a system in [`MettleSystems`], in `FixedPostUpdate`.
Systems in `FixedUpdate` run before it anyway. To read what it published in
the same tick, put a system after it:

```rust
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Resource, Default)] struct Seen(Vec<f32>);
fn drain(mut mana: Query<&mut Base<Mana>>) {
    for mut mana in &mut mana {
        mana.add(-10.0);
    }
}

fn see(mana: Query<&Current<Mana>>, mut seen: ResMut<Seen>) {
    seen.0.extend(mana.iter().map(|mana| mana.get()));
}

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
#   // One fixed step each update.
#   .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
#   .init_resource::<Seen>()
    .add_attribute::<Mana>()
    .add_systems(FixedUpdate, drain)
    .add_systems(FixedPostUpdate, see.after(MettleSystems));
# // The first update starts the clock, and runs no fixed step.
# app.update();
app.world_mut().spawn(Base::<Mana>::default());
app.update();

// Drained in the tick's FixedUpdate, and seen folded in the same tick.
assert_eq!(app.world().resource::<Seen>().0, [90.0]);
```

Systems in `Update` run after the fixed steps of the frame, so they see what
the last one published.

### Running it elsewhere

To run mettle in another schedule, set [`MettleCorePlugin::new`] as with
Bevy's own plugin groups:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins.set(MettleCorePlugin::new(Update))))
        .run();
}
```

Every part runs where it says. `MettleSystems` moves with it: order your
systems against it in that schedule.

## Using one part alone

To use one part alone, depend on its crate, such as
`bevy_mettle_attribute`, and add its plugin. Each part's crate depends on
the parts before it, not after.

Each part's plugin adds [`MettleCorePlugin`] if it isn't there yet. To run
mettle elsewhere, add `MettleCorePlugin::new(..)` before them. If you add the
plugins one by one, also add [`MettleContentPlugin`]. It loads
[`MettleContent`]; [Loading data](loading-data.md) shows how.

## Pitfalls

- **`add_attribute` before the plugins.** `add_attribute` adds
  `AttributePlugin` itself if it isn't there yet. `MettlePlugins` added
  after it adds it again, and Bevy panics. Add `MettlePlugins` first.
- **Time.** Effects run out by Bevy's time. `DefaultPlugins` and
  `MinimalPlugins` both include `TimePlugin`.
- **A frame without a fixed step.** Mettle runs in the fixed step, so a
  frame too short for one runs none of it. In a test, step time with
  `TimeUpdateStrategy::FixedTimesteps`, or run mettle in `Update`.
- **Lightyear.** With the `lightyear` feature, `MettlePlugins` also adds
  `MettleNetPlugin`, which needs lightyear's plugins. Add `MettlePlugins`
  after them. [Over lightyear](lightyear.md) shows how.

[`MettlePlugins`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettlePlugins.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.Mettle.html
[`MettleSet`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/enum.MettleSet.html
[`MettleSystems`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleSystems.html
[`MettleCorePlugin`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleCorePlugin.html
[`MettleCorePlugin::new`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleCorePlugin.html#method.new
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`MettleContentPlugin`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContentPlugin.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
