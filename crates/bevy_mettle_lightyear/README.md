# bevy_mettle_lightyear

[![Build Status][action-badge]][action-url]
[![Crate Docs][docs-badge]][docs-url]
[![Crate Version][crates-badge]][crates-url]

**Part of [bevy_mettle](https://github.com/declaredlost/bevy_mettle), a gameplay framework for Bevy.**

[action-badge]: https://img.shields.io/github/actions/workflow/status/declaredlost/bevy_mettle/build.yml?branch=main&label=build&logo=github&style=flat-square
[action-url]: https://github.com/declaredlost/bevy_mettle/actions/workflows/build.yml
[crates-badge]: https://img.shields.io/crates/v/bevy_mettle_lightyear.svg?logo=rust&style=flat-square
[crates-url]: https://crates.io/crates/bevy_mettle_lightyear
[docs-badge]: https://img.shields.io/docsrs/bevy_mettle_lightyear?logo=Docs.rs&style=flat-square
[docs-url]: https://docs.rs/bevy_mettle_lightyear

bevy_mettle over [lightyear](https://github.com/cBournhonesque/lightyear):
mettle's prediction, carried.

## Overview

- **Replicated, server to clients**: every active effect's replica and every
  granted ability's, as their actor is; and each attribute's base and served
  value the game asks for with [`replicate_attribute`]. A client shows them;
  it never decides them.
- **Sent**: a client's ability requests to the server, on [`MettleChannel`]
  (reliable, ordered), and the server's replies back to the client that
  asked.
- **Held**: a client holds a reply confirming its prediction until its
  actor's state as of the reply has arrived, so what it predicted gives way
  to the server's in the same frame: never paid twice, or not at all.
- **Checked**: the server takes a request only for an actor `ControlledBy`
  the client asking, and rejects any other back to it.
- **Owner-only**: an actor's effect and ability replicas go to the client
  controlling it alone; every client that sees it gets its cues, so its
  cooldowns and buffs stay its owner's (`Replicas::Everyone` to send them to
  all).

## Installation

Most games depend on `bevy_mettle` with its `lightyear` feature, where this
crate is `bevy_mettle::lightyear` and `MettlePlugins` adds its plugin:

```toml
[dependencies]
bevy_mettle = { version = "0.1", features = ["lightyear"] }
```

Or depend on this crate directly:

```toml
[dependencies]
bevy_mettle_lightyear = "0.1"
```

## Usage

The same on the server and its clients, after lightyear's plugins and
mettle's, with mettle's `NetRole` set:

```rust,no_run
use core::time::Duration;

use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy_mettle_ability::AbilityPlugin;
use bevy_mettle_attribute::prelude::*;
use bevy_mettle_effect::EffectPlugin;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_lightyear::prelude::*;
use lightyear::prelude::server::ServerPlugins;
use lightyear::prelude::*;

#[derive(Attribute)]
#[attribute(base = 100.0, min = 0.0)]
struct Mana;

/// On the server, an actor its client controls.
fn spawn(In(client): In<Entity>, mut commands: Commands) {
    commands.spawn((
        Base::<Mana>::default(),
        Replicate::to_clients(NetworkTarget::All),
        ControlledBy {
            owner: client,
            lifetime: Lifetime::default(),
        },
    ));
}

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins,
            StatesPlugin,
            ServerPlugins {
                tick_duration: Duration::from_millis(16),
            },
        ))
        .add_plugins((EffectPlugin::default(), AbilityPlugin))
        .insert_resource(NetRole::Server)
        .add_plugins(MettleNetPlugin::default())
        .add_attribute::<Mana>()
        .replicate_attribute::<Mana>()
        .run();
}
```

The game replicates its actors, and the owned tags it sets if clients read
them. Mettle's own state on them, the folded modifiers, is worked out on
each side.

## Learn more

- The book's [Over lightyear](https://declaredlost.github.io/bevy_mettle/lightyear.html)
  and [Prediction](https://declaredlost.github.io/bevy_mettle/prediction.html) pages
- The [API documentation](https://docs.rs/bevy_mettle_lightyear)

## Contributing

See [CONTRIBUTING.md](https://github.com/declaredlost/bevy_mettle/blob/main/CONTRIBUTING.md)
for development setup and guidelines.

## License

Licensed under either of [Apache License, Version 2.0](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-APACHE)
or [MIT license](https://github.com/declaredlost/bevy_mettle/blob/main/LICENSE-MIT) at your option.

[`replicate_attribute`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/trait.MettleNetApp.html#tymethod.replicate_attribute
[`MettleChannel`]: https://docs.rs/bevy_mettle_lightyear/latest/bevy_mettle_lightyear/struct.MettleChannel.html
