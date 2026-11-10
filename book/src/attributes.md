# Attributes

An attribute is a number an actor has: its life, its mana, its fire
resistance. In mettle each attribute is a type of yours. Its rules (where it
starts, what it's kept within) are written on the type.

The mage has six:

```rust
# use bevy_mettle::prelude::*;
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0, max = MaxLife)]
struct Life;

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxLife;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

#[derive(Attribute)]
#[attribute(max = MaxResistance)]
struct Resistance;

#[derive(Attribute)]
#[attribute(base = 0.75)]
struct MaxResistance;

#[derive(Attribute)]
#[attribute(base = 10.0)]
struct SpellPower;

assert_eq!(Life::NAME, "life");
assert_eq!(MaxResistance::NAME, "max_resistance");
```

## Settings

`#[attribute(..)]` takes these settings. Each is optional.

| Setting | Meaning | Unless it says |
|---|---|---|
| `base = 50.0` | What its base starts at | `0.0` |
| `min = 0.0`, `min = Other` | The least it can be: a number, or another attribute's current value | none |
| `max = ..` | The most it can be, like `min` | none |
| `rounding = Down`, `Up`, `Nearest` | It's a whole number, rounded that way | none |
| `on_limit = Clamp`, `Ratio` | What its base does as a limit that's another attribute moves | `Clamp` |
| `name = ".."` | The name data uses for it | the type's name in `snake_case` |

A limit that names another attribute reads that attribute's current value on
the same actor. `Life` can't go above the mage's `MaxLife`, whatever raises
`MaxLife`. The limits win over everything else: a modifier can't push `Life`
past them.

When such a limit moves, the base follows it, the same frame:

- **`Clamp`**, as it is unless it says: the base is kept within its limits.
  A mage at 100 life whose `MaxLife` drops to 80 has 80 life, and still 80
  as the maximum comes back. Taking a buff off and putting it on again
  doesn't heal.
- **`Ratio`**: the base keeps its fraction of its maximum, which has to be
  another attribute. Half life stays half as `MaxLife` rises or falls.

Only the world that decides attributes moves bases: a server, or a game on
its own. A client sees what the server serves. A base set by the game, with
`Base::set`, isn't kept within its limits; its current value is.

`Down` rounds 2.9 to 2. `Up` rounds 2.1 to 3. `Nearest` rounds halves away
from zero, so 2.5 is 3. Use rounding for counts, such as projectiles or
charges.

Data names an attribute by its name: `"life"`, `"max_resistance"`. Two
attribute types can't share a name. Adding the second one panics.

### Without the derive

The derive writes an [`Attribute`] impl, whose rules are constants. Write
the impl yourself if you'd rather:

```rust
# use bevy_mettle::prelude::*;
struct MaxLife;

impl Attribute for MaxLife {
    const NAME: &'static str = "max_life";
    const BASE: f32 = 100.0;
}

struct Life;

impl Attribute for Life {
    const NAME: &'static str = "life";
    const BASE: f32 = 100.0;
    const MIN: Option<Limit> = Some(Limit::Value(0.0));
    const MAX: Option<Limit> = Some(Limit::attribute::<MaxLife>());
}

assert_eq!(Life::NAME, "life");
```

## Adding them to the app

Add each attribute with [`add_attribute`]:

```rust,no_run
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MettlePlugins))
        .add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Mana>()
        .run();
}
```

Once it's added, data can name it, and every actor with its base has its
current value kept. Its components are registered for reflection, so
inspectors and scenes see them.

Adding it gives it its number, an [`AttributeId`]: what modifiers, actors'
state and the network know it by. Add attributes in the same order on a
server and its clients, as replication already asks of components, and the
numbers agree. [Names and numbers](concepts.md#names-and-numbers) says why.
Ask for a number with `world.attribute_id::<Life>()`, or from the
[`AttributeRegistry`] resource, which also gives each one's name and rules:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>();
let attributes = app.world().resource::<AttributeRegistry>();
let life = attributes.id_of::<Life>().unwrap();
assert_eq!(app.world().attribute_id::<Life>(), Some(life));
assert_eq!(attributes.name(life), Some("life"));
assert_eq!(attributes.id("max_life"), attributes.id_of::<MaxLife>());
```

`add_attribute` adds [`AttributePlugin`] if it isn't there yet. So add
`MettlePlugins` (or `AttributePlugin` itself) before the first
`add_attribute`. Adding the plugin twice panics, as with any Bevy plugin.

Attributes are folded in the [`Mettle`] schedule, run in `FixedPostUpdate`
by default, so what your systems change in a tick's `FixedUpdate` is folded
before the tick ends. To run it somewhere else, set the core plugin:
`MettlePlugins.set(MettleCorePlugin::new(Update))`.
[Installation](installation.md#where-mettle-runs) has more.

## Base and Current

An actor has an attribute when it has the attribute's [`Base`]. The base is
the value before any modifier: what's left of the mage's life, the mana it
has, a level it gained. It's yours to set, and effects set it too.

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component)] struct Mage;
fn spawn_mage(mut commands: Commands) {
    commands.spawn((
        Mage,
        Base::<Life>::default(),
        Base::<MaxLife>::default(),
        Base::<Mana>::new(60.0),
    ));
}
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins.set(MettleCorePlugin::new(Update))))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .add_attribute::<Mana>()
#     .add_systems(Startup, spawn_mage);
# app.update();
# let mana = app.world_mut().query::<&Current<Mana>>().single(app.world()).unwrap().get();
# assert_eq!(mana, 60.0);
```

`Base::default()` starts at the attribute's `base` setting. `Base::new`
starts it anywhere. [`Base::get`], [`Base::set`] and [`Base::add`] read and
change it.

A base brings the attribute's [`Current`]: the value with every modifier
that counts when nothing is in play, kept within its limits. It's the number
a health bar shows. Until it's first worked out, it's the base.

`Current` is mettle's to write. It's written only when the value changes,
so `Changed<Current<Life>>` means the mage's life changed:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Component)] struct HealthBar(f32);
fn health_bars(mut mages: Query<(&Current<Life>, &mut HealthBar), Changed<Current<Life>>>) {
    for (life, mut bar) in &mut mages {
        bar.0 = life.get();
    }
}
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins.set(MettleCorePlugin::new(Update))))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .add_systems(Update, health_bars.after(MettleSystems));
# let mage = app.world_mut().spawn((Base::<Life>::default(), Base::<MaxLife>::default(), HealthBar(0.0))).id();
# app.update();
# app.world_mut().get_mut::<Base<Life>>(mage).unwrap().set(40.0);
# app.update();
# assert_eq!(app.world().get::<HealthBar>(mage).unwrap().0, 40.0);
```

Changing a base doesn't change `Current` at once. It's worked out the next
time [`Mettle`] runs, and written in its last step. Put systems that read
`Current` after [`MettleSystems`].

Taking `Base<Life>` off the mage takes the attribute away, and its
`Current<Life>` with it.

## Told as it changes

As an actor's current value changes, mettle triggers
[`AttributeChanged<A>`] on it, with what it was (`old`) and what it is
(`new`). It's an entity event, so observe it on the actor or globally:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0, max = MaxLife)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0)] struct MaxLife;
# #[derive(Resource, Default)] struct Hurt(Vec<(f32, f32)>);
fn hurt(changed: On<AttributeChanged<Life>>, mut hurt: ResMut<Hurt>) {
    if changed.new < changed.old {
        hurt.0.push((changed.old, changed.new));
    }
}
# let mut app = App::new();
# app.add_plugins((MinimalPlugins, MettlePlugins.set(MettleCorePlugin::new(Update))))
#     .add_attribute::<Life>()
#     .add_attribute::<MaxLife>()
#     .init_resource::<Hurt>()
#     .add_observer(hurt);
# let mage = app.world_mut().spawn((Base::<Life>::default(), Base::<MaxLife>::default())).id();
# app.update();
# assert!(app.world().resource::<Hurt>().0.is_empty(), "the first value isn't a change");
# app.world_mut().get_mut::<Base<Life>>(mage).unwrap().set(70.0);
# app.update();
# assert_eq!(app.world().resource::<Hurt>().0, [(100.0, 70.0)]);
```

The first value worked out isn't a change, so it isn't told. Neither is a
value only some reads see, such as the mage's resistance to a fire hit:
[Reading in context](reading.md) covers those.

## On a client

In a multiplayer game, the server works out the values and a client shows
them. On a client, an actor marked [`Remote`] shows the values the server
serves as its `Current`, not its own fold of the modifiers sent to it.
[Prediction](prediction.md) explains how that works, and how a client's own
predictions show until the server answers.

## Pitfalls

- **Writing `Current`.** Set the base, or add a modifier. `Current` is
  overwritten the next time attributes are kept.
- **Reading too early.** A base set this tick shows in `Current` once
  `Mettle` has run. Order your systems after [`MettleSystems`].
- **The same name twice.** Two attribute types named alike (by the
  `name` setting or by their type names) panic when the second is added.

[`AttributeRegistry`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeRegistry.html
[`Attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/trait.Attribute.html
[`add_attribute`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/trait.AttributeApp.html#tymethod.add_attribute
[`AttributePlugin`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributePlugin.html
[`Base`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html
[`Base::get`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html#method.get
[`Base::set`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html#method.set
[`Base::add`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Base.html#method.add
[`Current`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.Current.html
[`AttributeChanged<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/registry/struct.AttributeChanged.html
[`Remote`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Remote.html
[`Mettle`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.Mettle.html
[`MettleSystems`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/schedule/struct.MettleSystems.html
