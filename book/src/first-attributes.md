# Your first attributes

This chapter gives the mage life and mana, and shows them changing.

## Declaring attributes

An attribute is a marker type with [`#[derive(Attribute)]`][`Attribute`].
Its rules go on `#[attribute(..)]`:

```rust
# use bevy_mettle::prelude::*;
/// What's left of the mage's life: never below 0, nor above its maximum.
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0, max = MaxLife)]
struct Life;

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxLife;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;
```

`base` is what a new actor starts with. `min` and `max` are limits: a
number, or another attribute. Life is capped by the mage's own maximum
life. [Attributes](attributes.md) lists every setting.

## Adding them

Add each attribute to the app with [`add_attribute`], after
`MettlePlugins`:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# fn spawn_mage() {}
# fn show_life() {}
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins))
        .add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Mana>()
        .add_systems(Startup, spawn_mage)
        .add_systems(Update, show_life)
        .run();
}
```

## The mage

An actor has an attribute when it has its [`Base`]. Spawn the mage with
one of each:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
#[derive(Component)]
struct Mage;

fn spawn_mage(mut commands: Commands) {
    commands.spawn((
        Mage,
        Base::<Life>::default(),
        Base::<MaxLife>::default(),
        Base::<Mana>::default(),
    ));
}
```

`Base::default()` starts at the attribute's `base`. `Base::new(80.0)`
starts elsewhere.

Each `Base<A>` brings a [`Current<A>`]: the value with every modifier
counted, kept within its limits. Your systems read it like any component:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
fn show_life(lives: Query<&Current<Life>, Changed<Current<Life>>>) {
    for life in &lives {
        info!("life: {}", life.get());
    }
}
```

`Current` is written only when its value changes, so
`Changed<Current<Life>>` is life changing.

## Changing a base

The base is yours to set: what's left of life after a hit, a level gained.
Change it through `Base::set` or `Base::add`. The next time attributes are
kept, `Current` follows, within its limits:

```rust
# use std::time::Duration;
# use bevy::prelude::*;
# use bevy::time::TimeUpdateStrategy;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     // Each update moves time on by one fixed step.
#     .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / 64.0)));
# app.finish();
# app.cleanup();
let mage = app
    .world_mut()
    .spawn((Base::<Life>::default(), Base::<MaxLife>::default()))
    .id();
# app.update();
# app.update();

// A hit for 30.
app.world_mut().get_mut::<Base<Life>>(mage).unwrap().add(-30.0);
# app.update();
assert_eq!(app.world().get::<Current<Life>>(mage).unwrap().get(), 70.0);

// Its maximum drops below what's left: life is capped.
app.world_mut().get_mut::<Base<MaxLife>>(mage).unwrap().set(50.0);
# app.update();
assert_eq!(app.world().get::<Current<Life>>(mage).unwrap().get(), 50.0);
```

The base keeps 70: the limit applies to `Current`. If maximum life comes
back up, so does life.

## Being told

As an actor's `Current<A>` changes, [`AttributeChanged<A>`] is triggered on
it, with what it was and what it is. Observe it for a health bar, or for
death:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
fn died(changed: On<AttributeChanged<Life>>, mut commands: Commands) {
    if changed.new <= 0.0 {
        commands.entity(changed.entity).despawn();
    }
}
```

Add it with `app.add_observer(died)`, or on the mage alone with
`commands.entity(mage).observe(died)`.

Next, [How it fits together](concepts.md) shows where effects and
abilities come in.

[`Attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/trait.Attribute.html
[`add_attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/trait.AttributeApp.html#tymethod.add_attribute
[`Base`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html
[`Current<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Current.html
[`AttributeChanged<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeChanged.html
