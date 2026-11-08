# Testing

Gameplay is easiest to test headless, one tick at a time. You give the mage
mana and the fireball, cast it, tick, and check what it paid and what was
told. [bevy_harness] does the ticking. Each tick moves time by one fixed
step, so tests are repeatable.

Add it as a dev-dependency:

```toml
[dev-dependencies]
bevy_harness = { git = "https://github.com/declaredlost/bevy_harness" }
```

## The fireball costs mana, and cools down

This test casts the fireball twice. The first cast pays 20 mana; the
second fails, since the first's cooldown still lasts. Two seconds on, it
can be cast again:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
use bevy_harness::TestApp;

# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Default, Behaviour)] struct CastFireball;
/// The mage's content: the fireball, its cost and its cooldown.
fn content() -> MettleContent {
    MettleContent {
        tags: ["cooldown", "cooldown.fireball"]
            .into_iter()
            .map(|id| Tag { id: Id::new(id) })
            .collect(),
        effects: vec![
            Effect::instant("fireball.cost")
                .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0)),
            Effect::lasting("fireball.cooldown", 2.0).grant("cooldown.fireball"),
        ],
        abilities: vec![
            Ability::new("fireball", "cast_fireball")
                .cost("fireball.cost")
                .cooldown("fireball.cooldown"),
        ],
        ..default()
    }
}

/// An app with the mage, granted the fireball.
fn mage_app() -> (TestApp, Entity) {
    let mut app = TestApp::builder()
        .plugins(MettlePlugins)
        .with(|app| {
            app.add_attribute::<Mana>()
                .add_behaviour::<CastFireball>()
                .insert_resource(content());
        })
        .watch_triggered::<ContentRejected>()
        .watch_triggered::<AbilityFailed>()
        .build();
    app.assert_triggered_none::<ContentRejected>(|_| true);
    let mage = app.spawn(Base::<Mana>::default());
    app.world_mut().grant_ability(mage, "fireball", 1.0).unwrap();
    app.tick();
    (app, mage)
}

# fn main() {
// #[test]
// fn the_fireball_costs_mana_and_cools_down() {
let (mut app, mage) = mage_app();

// Cast: 20 mana paid.
let fireball = app.world_mut().activate_ability(mage, "fireball").unwrap();
app.tick();
app.entity(mage).assert::<Current<Mana>>(|mana| mana.get() == 80.0);

// It's running, as a `CastFireball` on the ability.
app.entity(fireball).assert_has::<CastFireball>();

// End it, and cast again: still cooling down.
app.world_mut().end_ability(fireball);
app.world_mut().activate_ability(mage, "fireball").unwrap_err();
app.assert_triggered::<AbilityFailed>(|failed| failed.reason == Failure::Cooldown);

// Two seconds on, it can be cast again.
app.secs(2.0);
app.world_mut().activate_ability(mage, "fireball").unwrap();
app.tick();
app.entity(mage).assert::<Current<Mana>>(|mana| mana.get() == 60.0);
// }
# }
```

In your crate, the body is a `#[test]` function. `activate_ability` on the
world returns the granted ability's entity, or why it didn't activate. Its
behaviour's component is on that entity while it's active.

## What helps

- **Content in code.** The definitions' builders (`Effect::instant`,
  `Ability::new`) make a test's content short. Check the game's real files
  in a test of their own: read them into `MettleContent`, and assert
  `check` holds against an app with the game's code added.
- **Watch what's told.** `watch_triggered::<AbilityFailed>()` records every
  failure, so `assert_triggered` can say why a cast didn't happen. The same
  works for `AttributeChanged<A>`, `EffectApplied` and the rest.
- **Tick before reading.** A cost lowers `Base<Mana>` as it's paid;
  `Current<Mana>` follows the next time attributes are kept. Tick once
  before asserting on `Current`.
- **Seconds, not ticks.** `app.secs(2.0)` runs as many ticks as two seconds
  take, so a test doesn't depend on the tick rate.

[bevy_harness]: https://github.com/declaredlost/bevy_harness
