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
  `EffectOf` and `AbilityOf`, as their actor is (`ReplicateLike` it: to
  whom it's visible, with it); and each actor's `Base<A>` and
  `Served<A>` the game asks for with `replicate_attribute`. On the server
  every actor with `Base<A>` serves `A`; on the client every actor
  replicated from the server is `Remote`, so it shows the server's values
  rather than its own fold, but while it predicts. An effect's source is
  mapped. A client shows them; it never decides them.
- **Sent**: a client's `AbilityRequest`s to the server, on
  [`MettleChannel`] (reliable, ordered); the server's `AbilityReply` back
  to the client that asked, with the replication tick its state goes out
  at. A reply isn't ordered with replication, so a client holds one
  confirming its prediction until its actor's state as of it has arrived
  (or 30 frames, for one that changed nothing): what it predicted gives
  way to the server's in the same frame, never paid twice or not at all.
- **Checked**: the server takes a request only for an actor
  `ControlledBy` the client asking. Any other is rejected back to it (as
  `NotGranted`, so its prediction is undone), with a warning.

The game replicates its actors (and their owned tags it sets, if clients
read them); mettle's own state on them, folded modifiers, is worked out
on each side, and a client's reads in context (a hit's tags) are its own,
advisory.
