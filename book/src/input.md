# Input and targets

The fireball goes where the player aims it. Where it's aimed isn't data and
isn't the cast's own state: it's what the player chose as they cast it, the
ability's *input*. This chapter gives the mage an aim.

## The input

Input is a component of yours. It holds what the player chose, and nothing
the cast keeps for itself:

```rust
# use bevy::prelude::*;
/// Where the fireball is aimed: a point in the world.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
struct Aim(Vec2);
```

A behaviour says which input it takes, with `input` on `#[behaviour(..)]`.
Its own fields stay its run state:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Clone, Copy, Debug, PartialEq)] struct Aim(Vec2);
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct CastFireball {
    timer: f32,
}
```

A target entity is input too: `struct Target(Entity)`. So is a charge time,
or the step of a combo: whatever the player chose that the cast runs on.

## Activating with it

An ability whose behaviour takes input is activated with it, by
[`activate_ability_with`], naming the behaviour: its input is the type the
behaviour says, so giving the fireball anything but an `Aim` doesn't compile.
The input is put on the ability's entity next to the behaviour, and taken off
with it as the ability ends. Your systems read both:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Clone, Copy, Debug, PartialEq)] struct Aim(Vec2);
# #[derive(Component, Default, Behaviour)] #[behaviour(input = Aim)] struct CastFireball { timer: f32 }
# #[derive(Ability)] struct Fireball;
/// Where the player clicked this frame, if they did: kept by your input code.
#[derive(Resource)]
struct Clicked(Option<Vec2>);

#[derive(Component)]
struct Player;

/// Casts the fireball where the player clicked.
fn cast_at(clicked: Res<Clicked>, player: Single<Entity, With<Player>>, mut commands: Commands) {
    if let Some(at) = clicked.0 {
        commands.entity(*player).activate_ability_with::<CastFireball>(Fireball, Aim(at));
    }
}

/// Runs each cast, along its aim.
fn cast(mut casts: Query<(&ActiveAbility, &mut CastFireball, &Aim)>, time: Res<Time>) {
    for (active, mut cast, aim) in &mut casts {
        cast.timer += time.delta_secs();
        let _ = (active.actor, aim.0);
    }
}
```

Which behaviour runs an ability is data, so that the fireball is run by
`CastFireball` is checked as it activates. If it's run by another, or it's
activated with `activate_ability` and no input, it fails with
[`Failure::Input`], and nothing is paid.

## Checking it

A player can ask for anything: a fireball aimed across the map, at a target
behind a wall. Register a check for the input with [`check_input`]. It's a
system, given an [`InputCheck`]: the input, the actor and the ability. It can
ask anything a system can, a spatial query for line of sight too:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Attribute)] #[attribute(base = 100.0, min = 0.0)] struct Mana;
# #[derive(Component, Clone, Copy, Debug, PartialEq)] struct Aim(Vec2);
# #[derive(Component, Default, Behaviour)] #[behaviour(input = Aim)] struct CastFireball { timer: f32 }
# #[derive(Ability)] struct Fireball;
/// Whether the aim is within 12 of the mage.
fn in_reach(In(check): In<InputCheck<Aim>>, mages: Query<&Transform>) -> bool {
    mages
        .get(check.actor)
        .is_ok_and(|at| at.translation.truncate().distance(check.input.0) <= 12.0)
}

let mut app = App::new();
app.add_plugins((MinimalPlugins, MettlePlugins))
    .add_attribute::<Mana>()
    .add_behaviour::<CastFireball>()
    .check_input::<Aim, _>(in_reach)
    .add_ability::<Fireball>()
    .insert_resource(MettleContent {
        effects: vec![EffectData::instant("fireball.cost")
            .modifier(EffectModifier::new::<Mana>(Operation::Flat, -20.0))],
        abilities: vec![AbilityData::new(Key::of::<Fireball>(), Key::of::<CastFireball>()).cost("fireball.cost")],
        ..default()
    });
app.update();

let world = app.world_mut();
let mage = world.spawn((Base::<Mana>::default(), Transform::default())).id();
world.grant_ability(mage, Fireball).unwrap();

// Too far: rejected before anything's paid.
assert_eq!(
    world.activate_ability_with::<CastFireball>(mage, Fireball, Aim(Vec2::new(40.0, 0.0))),
    Err(Failure::Input)
);
// Within reach: cast.
let fireball = world
    .activate_ability_with::<CastFireball>(mage, Fireball, Aim(Vec2::new(8.0, 0.0)))
    .unwrap().ability();
assert_eq!(world.get::<Aim>(fireball), Some(&Aim(Vec2::new(8.0, 0.0))));
```

The check runs as the ability activates, before its cost is paid or its
behaviour put on, wherever it activates: a client predicting, the server
deciding, a world on its own. Your game's AI is held to the same check.

## New input while it runs

Some abilities take input as they run: a beam that follows the cursor, a
charge released at a spot. Give the active ability new input with
[`set_input`], naming its behaviour again. It replaces what it had:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Component, Clone, Copy, Debug, PartialEq)] struct Aim(Vec2);
/// A beam being channelled, along its aim.
#[derive(Component, Default, Behaviour)]
#[behaviour(input = Aim)]
struct Beam;

/// Where the cursor is: kept by your input code.
#[derive(Resource)]
struct Cursor(Vec2);

/// Keeps each beam on the cursor.
fn follow(beams: Query<Entity, With<Beam>>, cursor: Res<Cursor>, mut commands: Commands) {
    for beam in &beams {
        commands.entity(beam).set_input::<Beam>(Aim(cursor.0));
    }
}
```

New input is checked the same way, where the ability is decided: on a
server, or a world on its own. Input that doesn't pass cancels the ability,
and [`AbilityFailed`] says [`Failure::Input`].

## Over a network

A client predicts with its own input. As it activates with
`activate_ability_with`, it runs the cast at once, and asks the server with
the input, as an [`AbilityRequestWith`]. New input from `set_input` is
predicted and sent as an [`AbilityInput`]. Both go in order, after the
activation they're for.

The server checks the input before it activates anything. If it doesn't
hold, the activation is rejected, and the client undoes what it predicted,
its cost and cooldown too. New input that doesn't hold cancels the ability on
the server, and the rejection cancels it on the client. So a client never
gets to run a cast the server wouldn't. [Prediction](prediction.md) has the
rest of how the two agree, and [Over lightyear](lightyear.md) how input is
sent.

Other clients aren't sent input. What they see of the cast comes from what
it applies.

## Pitfalls

- **Input isn't run state.** Keep what the cast tracks (its timer, how many
  bolts it has fired) on the behaviour, not the input. The server takes the
  input from the client as it is, after its check.
- **Triggers give no input.** An ability whose behaviour takes input can't
  have triggers: a gameplay event or a tag has no aim to give it. Content
  with one is rejected. Activating such an ability by tag fails with
  `Failure::Input`.
- **A check is the server's word.** On a client, a check only spares a
  prediction the server would refuse. Write it from what the server knows:
  where actors are on the server, not where the client last saw them.

[`activate_ability_with`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.AbilityCommands.html#tymethod.activate_ability_with
[`set_input`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/trait.AbilityCommands.html#tymethod.set_input
[`check_input`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/trait.BehaviourApp.html#tymethod.check_input
[`InputCheck`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/behaviour/struct.InputCheck.html
[`Failure::Input`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/enum.Failure.html#variant.Input
[`AbilityFailed`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/events/struct.AbilityFailed.html
[`AbilityRequestWith`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityRequestWith.html
[`AbilityInput`]: https://docs.rs/bevy_mettle_ability/latest/bevy_mettle_ability/net/struct.AbilityInput.html
