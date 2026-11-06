# bevy_mettle_lightyear

`bevy_mettle` over [lightyear](https://github.com/cBournhonesque/lightyear):
mettle's prediction, carried.

```rust,ignore
// The same on the server and its clients, after lightyear's plugins and mettle's:
app.insert_resource(NetRole::Server) // or NetRole::Client
    .add_plugins(MettleNetPlugin)
    .replicate_attribute::<Life>()
    .replicate_attribute::<Mana>();

// On the server, an actor its client controls:
commands.spawn((
    Base::<Life>::default(),
    Base::<Mana>::default(),
    Replicate::to_clients(NetworkTarget::All),
    ControlledBy { owner: client_link, lifetime: Lifetime::default() },
));
```

- **Replicated, server to clients**: every active effect's
  `EffectReplica` and every granted ability's `AbilityReplica`, with their
  `EffectOf` and `AbilityOf`; and each actor's `Base<A>` the game asks
  for. A client shows them; it never decides them.
- **Sent**: a client's `AbilityRequest`s to the server, on
  [`MettleChannel`] (reliable, ordered); the server's `AbilityReply` back
  to the client that asked.
- **Checked**: the server takes a request only for an actor
  `ControlledBy` the client asking. Any other is rejected back to it (as
  `NotGranted`, so its prediction is undone), with a warning.

The game replicates its actors (and their owned tags it sets, if clients
read them); mettle's own state on them, folded modifiers and current
values, is worked out on each side.
