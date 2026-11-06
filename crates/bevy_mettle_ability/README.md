# bevy_mettle_ability

`bevy_mettle`'s abilities, as in Unreal's Gameplay Ability System: what
an actor can do, granted to it, activated, paid for, cooled down, and
ended. What one is, what it costs and what blocks it is data; what it
does is a behaviour in code.

```rust,ignore
#[derive(Component, Default)]
struct ProjectileCast { timer: f32 }

app.add_plugins(AbilityPlugin)
    .add_behaviour::<ProjectileCast>("projectile_cast")
    .insert_resource(AbilityRegistry::build(&abilities, &grants, &tags, &effects, &behaviours)?);

commands.entity(mage).grant_ability("fireball", 1.0);
commands.entity(mage).activate_ability("fireball");

// The game's systems run it, across frames, until it ends:
fn cast(mut casting: Query<(Entity, &ActiveAbility, &mut ProjectileCast)>, time: Res<Time>, mut commands: Commands) {
    for (ability, active, mut cast) in &mut casting {
        cast.timer += time.delta_secs();
        if cast.timer > 0.5 {
            commands.entity(ability).end_ability();
        }
    }
}
```

At the root: [`AbilityPlugin`], [`Ability`], [`AbilityRegistry`],
[`AbilityCommands`] and [`AbilityWorld`]. The rest is in four modules, and
[`prelude`] brings in what using abilities takes:

| Module | What's in it |
|---|---|
| [`data`] | An ability's parts, and what runs it: `Grant`, `Commit`, `Trigger`, `Behaviour`, `Behaviours` |
| [`active`] | Granted and active abilities: `Abilities`, `AbilityOf`, `GrantedAbility`, `ActiveAbility` |
| [`events`] | What's told: `AbilityActivated`, `AbilityEnded`, `AbilityFailed`, `GameplayEvent` |
| [`net`] | Over a network: `AbilityRequest`, `AbilityReply`, `AbilityReplica` |

## Definitions

An ability is data, named by an `Id<Ability>`: [`Ability`].

```ron
(
    id: "fireball",
    tags: ["ability.spell.fire"],
    behaviour: "projectile_cast",
    cost: "fireball.cost",
    cooldown: "fireball.cooldown",
    require: Not(OwnerHas("state.silenced")),
    blocked_by: ["state.stunned"],
    cancels: ["ability.channel"],
    blocks: ["ability.spell"],
    grants: ["state.casting"],
    triggers: [Event("event.parried")],
    commit: OnActivate,
)
```

| Field | What it does |
|---|---|
| `behaviour` | The component in code that runs it, added with `add_behaviour` |
| `cost` | An instant effect it pays as it commits; it can't activate if paying would take an attribute below 0 |
| `cooldown` | A lasting effect applied as it commits; it can't activate while its owner owns what that grants |
| `require`, `blocked_by` | What has to hold; what its owner can't own |
| `cancels`, `blocks` | Active abilities tagged so cancelled as it activates; others tagged so kept from activating while it's active |
| `grants` | Tags its owner owns while it's active |
| `triggers` | `Event(tag)`, `TagAdded(tag)`, `TagPresent(tag)` (cancelled as the tag goes) |
| `commit` | `OnActivate`, or `Manual`: the behaviour commits (after a cast time), cancelled if it can't pay then |

[`AbilityRegistry::build`] checks every ability against the tag tree, the
effects and the behaviours there are, and every grant: every error at
once.

## Grants

What an effect grants is the ability crate's part of it, as Unreal's
abilities component is: an effect knows nothing of abilities.

```ron
[(effect: "staff", abilities: ["channel"])]
```

Equipping the staff applies its effect, which grants channel; taking it
off takes it away. [`AbilityRegistry::build`] checks each grant: its
effect is one there is, and lasts; its abilities are ones there are. The
game can keep an item's effect and its grant in one file of its own, and
hand mettle the two lists.

## Granted and active

A granted ability is an entity [`AbilityOf`] its actor, which despawns
with it: granted by `grant_ability`, or by an effect for as long as it
lasts, through a [`Grant`](data::Grant): gear's skills. Activating it
checks it, in order: granted and not active, not cooling down, affordable,
not blocked, its requirement holding. Then abilities it cancels are
cancelled, its tags granted, its behaviour's component put on it with
[`ActiveAbility`] (its actor, activation, level, and the event that
triggered it), and, unless it commits by hand, its cost and cooldown
applied. Ending it takes them off.

[`AbilityActivated`], [`AbilityFailed`] (why) and [`AbilityEnded`] are
entity events on the actor. [`GameplayEvent`] (a tag, an instigator, a
target, a magnitude) is sent to an actor to activate what it triggers, and
told to observers too.

## Over a network

With the effect crate's `NetRole` set:

- **A client predicts its own abilities**: `activate_ability` activates at
  once under a new activation (its behaviour running, its cost and
  cooldown predicted), and writes an [`AbilityRequest`] for the game to
  send to the server.
- **The server decides**: it activates what [`AbilityRequest`]s ask for,
  under the client's activation, and answers each with an
  [`AbilityReply`].
- **The client resolves**: a confirmed activation's predictions stay
  until the server's own replace them; a rejected one is ended and all of
  it undone, and [`AbilityFailed`] says why.
- **Granted abilities replicate** as [`AbilityReplica`]s with their
  [`AbilityOf`]. A client leaves triggers, and abilities effects grant,
  to the server.

A server's own activations have the top bit set, apart from its clients'.

[`AbilityActivated`]: events::AbilityActivated
[`AbilityEnded`]: events::AbilityEnded
[`AbilityFailed`]: events::AbilityFailed
[`AbilityOf`]: active::AbilityOf
[`AbilityReplica`]: net::AbilityReplica
[`AbilityReply`]: net::AbilityReply
[`AbilityRequest`]: net::AbilityRequest
[`ActiveAbility`]: active::ActiveAbility
[`GameplayEvent`]: events::GameplayEvent
