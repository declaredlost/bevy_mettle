# Over lightyear

[Prediction](prediction.md) said what to send. `bevy_mettle_lightyear` sends
it over [lightyear](https://github.com/cBournhonesque/lightyear): replicas
replicated, requests and replies carried, and who may ask checked.

Turn on the `lightyear` feature:

```toml
[dependencies]
bevy_mettle = { version = "0.1", features = ["lightyear"] }
```

It's then `bevy_mettle::lightyear`, its names are in the prelude, and
[`MettlePlugins`] adds [`MettleNetPlugin`] too.

## Setting up

Both sides add the same plugins, in the same order:

1. lightyear's own plugins: `ServerPlugins` on the server, `ClientPlugins` on
   a client;
2. [`MettlePlugins`], with [`NetRole`] set to `Server` or `Client`;
3. your attributes, and [`replicate_attribute`] for each one clients should
   see;
4. your behaviours, and [`replicate_input`] for each input they take.

```rust
# use core::time::Duration;
# use bevy::prelude::*;
# use bevy::state::app::StatesPlugin;
# use bevy_mettle::prelude::*;
# use lightyear::prelude::client::ClientPlugins;
# use lightyear::prelude::server::ServerPlugins;
#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0, max = MaxLife)]
struct Life;

#[derive(Attribute)]
#[attribute(base = 100.0)]
struct MaxLife;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

/// Where the fireball is aimed: sent from the client as it is.
#[derive(Component, Clone, Copy, serde::Serialize, serde::Deserialize, bevy::ecs::entity::MapEntities)]
struct Aim(Vec2);

#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct CastFireball;

const TICK: Duration = Duration::from_millis(16);

/// What both sides add, after lightyear's plugins.
fn mettle(app: &mut App, role: NetRole) {
    app.insert_resource(role)
        .add_plugins(MettlePlugins)
        .add_attribute::<Life>()
        .add_attribute::<MaxLife>()
        .add_attribute::<Mana>()
        .replicate_attribute::<Life>()
        .replicate_attribute::<MaxLife>()
        .replicate_attribute::<Mana>()
        .add_behaviour::<CastFireball>()
        .replicate_input::<Aim>();
}

let mut server = App::new();
server.add_plugins((MinimalPlugins, StatesPlugin, ServerPlugins { tick_duration: TICK }));
mettle(&mut server, NetRole::Server);

let mut client = App::new();
client.add_plugins((MinimalPlugins, StatesPlugin, ClientPlugins { tick_duration: TICK }));
mettle(&mut client, NetRole::Client);
```

[`MettleNetPlugin`] registers its channel, messages and components with
lightyear, which only works once lightyear's plugins are there. With the
`lightyear` feature on, adding [`MettlePlugins`] to an app without them
panics. If the same game also runs offline, leave the net plugin out there:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
let mut offline = App::new();
offline.add_plugins(MettlePlugins.build().disable::<MettleNetPlugin>());
```

Set the [`NetRole`] before the game starts. The plugin does what that role
needs, and nothing for the other.

## Your actors

Your game replicates its actors. Mettle replicates what's on them. On the
server, the mage its player controls:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use lightyear::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Life;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
fn spawn_mage(commands: &mut Commands, player: Entity) -> Entity {
    commands
        .spawn((
            Base::<Life>::default(),
            Base::<Mana>::default(),
            Replicate::to_clients(NetworkTarget::All),
            ControlledBy {
                owner: player,
                lifetime: Lifetime::default(),
            },
        ))
        .grant_ability("fireball", 1.0)
        .id()
}
# let _ = spawn_mage;
```

`player` is the client's link on the server. [`ControlledBy`] is what lets
that client cast the mage's abilities. Without it, the client sees the mage
but can't act for it.

## What's replicated

From the server to its clients:

- every active effect's [`EffectReplica`] and every granted ability's
  [`AbilityReplica`], with their [`EffectOf`] and [`AbilityOf`], to the
  client [`ControlledBy`] the actor alone: it predicts with them. They go
  and come with the actor (`ReplicateLike` it), and a visibility filter
  keeps them to its owner, following control as it comes, changes hands or
  goes. An actor no client controls sends them to none. Set
  `MettleNetPlugin { replicas: Replicas::Everyone }` to send them to every
  client that sees the actor, for a party frame showing allies' buffs;
- each actor's [cues](cues.md), lasting and one-shot, to every client that
  sees it: what other clients see of its effects and abilities, while its
  cooldowns and buffs stay its owner's;
- each actor's `Base<A>` and [`Served<A>`], for the attributes you named with
  [`replicate_attribute`]. `Base<A>` is what the client works predictions out
  on. `Served<A>` is what it shows. On the server, every actor with `Base<A>`
  serves `A`;
- an effect's source, mapped to the client's entity.

On a client, every actor replicated from the server is marked [`Remote`], so
it shows the server's values rather than its own fold, except while it
predicts. A client shows all this. It never decides it.

Mettle's own state on an actor, its folded modifiers, isn't replicated. It's
worked out on each side. If clients read an actor's owned tags that your game
sets, replicate those yourself. So too an item's modifiers, if clients should
fold them:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# use lightyear::prelude::*;
fn replicate_items(app: &mut App) {
    app.component::<Modifiers>().replicate();
    app.component::<ModifierOf>().replicate();
}
# let _ = replicate_items;
```

A client's reads in context, a hit's tags say, are its own, worked out from
what it has. They're advisory.

## Requests and replies

- **Asking.** A client's [`AbilityRequest`]s go to the server on
  [`MettleChannel`], reliable and ordered, and with [`replicate_input`] its
  requests with input and its new input, in order after them. An input is
  sent as it is, so it derives `Serialize`, `Deserialize` and `MapEntities`
  (a target entity is mapped to the server's).
- **Checking.** The server takes a request, or input, only for an actor
  [`ControlledBy`] the client sending it. A request for any other is
  rejected back to that client as `Failure::NotGranted`, with a warning, so
  its prediction is undone; input for any other is dropped.
- **Answering.** The server's [`AbilityReply`] goes to the client
  controlling the actor, the only one it takes requests from, with the
  replication tick its state goes out at. Later replies, rejecting a running
  activation's input, go the same way.
- **Ending.** A client's cancels of its own activations go to the server
  after the requests they cancel, taken only for actors it controls; the
  server's ends of a client's activations go to the client controlling the
  actor. Both on [`MettleChannel`].

A reply isn't ordered with replication: they travel different ways. So a
client holds a reply that confirms its prediction until the actor's state as
of that tick has arrived. Then the prediction gives way to the server's state
in the same frame. The cost is never paid twice, or not at all. A confirming
reply whose actor never changed has nothing to wait for, so it's held for 30
frames at most. A rejecting reply is handed over at once, since nothing
changed on the server.

Replies and requests are heard in `PreUpdate`, after lightyear receives its
messages and replication, and before [`AbilitySystems`].

## Pitfalls

- **Plugin order.** lightyear's plugins, then [`MettlePlugins`]. The other way
  round panics.
- **The same protocol.** Both sides register the same attributes with
  [`replicate_attribute`], and the same components, in the same order.
- **The same data.** Effects and abilities are named, not sent. Both sides
  need the same registries.
- **Control.** A mage without [`ControlledBy`] its player's link can't be
  cast for: every request is rejected.

[`AbilityOf`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/active/struct.AbilityOf.html
[`AbilityReplica`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/replica/struct.AbilityReplica.html
[`AbilityReply`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/request/struct.AbilityReply.html
[`AbilityRequest`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/request/struct.AbilityRequest.html
[`AbilitySystems`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/struct.AbilitySystems.html
[`ControlledBy`]: https://docs.rs/lightyear_replication/latest/lightyear_replication/control/struct.ControlledBy.html
[`EffectOf`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/active/struct.EffectOf.html
[`EffectReplica`]: https://docs.rs/bevy_mettle_effect/latest/bevy_mettle_effect/net/struct.EffectReplica.html
[`MettleChannel`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/struct.MettleChannel.html
[`MettleNetPlugin`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/struct.MettleNetPlugin.html
[`MettlePlugins`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettlePlugins.html
[`NetRole`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/net/enum.NetRole.html
[`Remote`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Remote.html
[`replicate_input`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/trait.MettleNetApp.html#tymethod.replicate_input
[`replicate_attribute`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/trait.MettleNetApp.html#tymethod.replicate_attribute
[`Served<A>`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/authority/struct.Served.html
