# Gameplay tags

A gameplay tag is a label in a tree: `damage.elemental.fire` sits under
`damage.elemental`, which sits under `damage`. Everything in mettle is
labelled with tags. The mage's fireball is tagged `ability.spell.fire`, the
burn it leaves is `effect.affliction.burning`, and while it burns the mage
owns `state.burning`.

What has a tag has every tag above it. A rule written for `damage.elemental`
holds for a fire hit, and a cure for `effect.affliction` takes off burning
and poison alike. This is how Unreal's gameplay tags work too.

## The tag tree

Tags are data. You define them, usually in a file, and a mod can add its
own. Each tag is a [`Tag`] with an id, its path:

```ron
[
    (id: "damage"),
    (id: "damage.elemental"),
    (id: "damage.elemental.fire"),
    (id: "state"),
    (id: "state.burning"),
    (id: "state.wet"),
]
```

In a game, the tags go in [`MettleContent`]'s `tags`, and mettle builds the
tree as the content is loaded, keeping it as a resource, `Res<TagTree>`.
[`TagTree::build`] is that step on its own: it checks the definitions and
gives you the tree everything else is checked against:

```rust
# use bevy_mettle::prelude::*;
let file = r#"[
    (id: "damage"),
    (id: "damage.elemental"),
    (id: "damage.elemental.fire"),
    (id: "state"),
    (id: "state.burning"),
    (id: "state.wet"),
]"#;
let definitions: Vec<Tag> = ron::from_str(file).unwrap();
let tags = TagTree::build(&definitions).unwrap();

assert!(tags.contains(&Id::from_static("damage.elemental.fire")));
assert!(!tags.contains(&Id::from_static("damage.poison")));
```

`build` takes definitions from however many files at once: chain them, or
collect them into one list. It reports every mistake together, not just the
first:

- **A tag defined twice.**
- **A tag that isn't a path.** A path is words joined by `.`, each word made
  of `a` to `z`, `0` to `9` and `_`. `Fire` and `damage..fire` aren't tags.
- **A gap.** Every tag's parent has to be a tag of its own. Defining
  `damage.elemental.fire` without `damage.elemental` is an error.

```rust
# use bevy_mettle::prelude::*;
let definitions = [
    Tag { id: Id::from_static("damage") },
    Tag { id: Id::from_static("damage.elemental.fire") },
];
let errors = TagTree::build(&definitions).unwrap_err();
assert_eq!(
    errors.to_string(),
    "there's no tag called `damage.elemental` (in `damage.elemental.fire`)"
);
```

Mettle doesn't read files. Your game loads them however it likes, and hands
the definitions over. [Loading data](loading-data.md) shows one way.

## Naming a tag

In code, a tag is an `Id<Tag>`. An [`Id`] is a name typed by what it names,
so a tag's id can't be passed where an effect's is expected. Make one with
`Id::from_static("state.burning")`, as a constant, or `Id::new(name)` from a
string. Most of mettle's builders take `impl Into<Id<Tag>>`, so a plain
`"state.burning"` works there too.

An id knows where it sits in the tree:

```rust
# use bevy_mettle::prelude::*;
let fire: Id<Tag> = Id::from_static("damage.elemental.fire");

assert_eq!(fire.parent(), Some(Id::from_static("damage.elemental")));
assert!(fire.is_within(&Id::from_static("damage")));
assert!(!fire.is_within(&Id::from_static("damage.elem")));

let lineage: Vec<String> = fire.lineage().map(|tag| tag.to_string()).collect();
assert_eq!(lineage, ["damage.elemental.fire", "damage.elemental", "damage"]);
```

## Sets of tags

[`Tags`] is a set: what an effect, an ability or a hit is labelled with. It
holds every tag above the ones you give it, so asking it for `effect` finds
`effect.affliction.burning`:

```rust
# use bevy_mettle::prelude::*;
let burning: Tags = [Id::from_static("effect.affliction.burning")]
    .into_iter()
    .collect();

assert!(burning.contains(&Id::from_static("effect.affliction")));
assert!(burning.contains(&Id::from_static("effect")));
// Written back as given, without what's above.
assert_eq!(burning.written().count(), 1);
```

In data, a set is a list of the tags as given: `["effect.affliction.burning"]`.

## Owned tags

An actor's states are tags it owns: [`OwnedTags`], a component. The game
marks the mage `state.casting` while it casts. Effects grant tags too: while
the mage burns, it owns `state.burning`. Every actor with an attribute has
`OwnedTags`.

Owned tags are counted. Each source adds one and later takes its one away.
If two effects both grant `immune.fire`, the mage stays immune until both
are gone:

```rust
# use bevy_mettle::prelude::*;
let immune = Id::from_static("immune.fire");
let mut owned = OwnedTags::default();
owned.add(&immune); // A potion.
owned.add(&immune); // A ring.

owned.remove(&immune); // The potion wears off.
assert!(owned.contains(&immune));
assert!(owned.contains(&Id::from_static("immune")));

owned.remove(&immune); // The ring comes off.
assert!(!owned.contains(&immune));
```

Removing a tag only takes away what was added as that tag. Owning
`immune.fire` means owning `immune`, but `owned.remove(&immune)` with
`immune` does nothing: no source added `immune` itself. `OwnedTags` reads
as a [`TagCounts`], which holds the counts: `count(&tag)` says how many
sources hold a tag, and `iter()` lists them.

### Told as they come and go

When an actor comes to own a tag, [`TagAdded`] is triggered on it. When it
stops, [`TagRemoved`] is. Each tag above counts too: the first fire
immunity tells both `immune.fire` and `immune`. Observe them on the actor,
or globally:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Resource, Default)]
struct Told(Vec<String>);

let mut app = App::new();
// Attributes are kept in `Update` here, so one update tells them.
app.add_plugins(MettlePlugins.set(AttributePlugin::new(Update)))
    .init_resource::<Told>()
    .add_observer(|added: On<TagAdded>, mut told: ResMut<Told>| {
        told.0.push(format!("+{}", added.tag));
    })
    .add_observer(|removed: On<TagRemoved>, mut told: ResMut<Told>| {
        told.0.push(format!("-{}", removed.tag));
    });

let mage = app.world_mut().spawn(OwnedTags::default()).id();
app.world_mut()
    .get_mut::<OwnedTags>(mage)
    .unwrap()
    .add(&Id::from_static("state.burning"));
app.update();

assert_eq!(app.world().resource::<Told>().0, ["+state.burning", "+state"]);
```

They're told as the [`publish_tags`] system runs, in the attribute plugin's
[`AttributeSystems`], before values are worked out. Changes are settled
first: a tag added and taken away again before then isn't told at all.

### Asking from a behavior tree

[`owns`] is a guard: a read-only system that takes an agent and a tag, and
says whether the agent owns the tag or one under it. It has the shape
`bevy_arbor` takes for a decorator, so a tree can wait on `state.stunned`.

## Checking a client has the server's tags

Tags travel as names. A client and its server have to agree on the tree.
[`TagTree::fingerprint`] is a hash of the tags that doesn't depend on the
order they were defined in. Send it on connecting and compare.

## Pitfalls

- **Ids aren't checked where you write them.** `Id::from_static("stat.burning")`
  compiles. The mistake is caught when data naming it is checked against the
  tree, as effects and abilities are when content is loaded.
- **Facts are tags too.** [Conditions](conditions.md) name the numbers a read
  supplies by tags, such as `fact.flown`. Define them in the tree, or
  conditions naming them fail their checks.
- **No events without the attribute plugin.** `publish_tags` runs in
  `AttributeSystems`. Without `AttributePlugin`, owned tags still count, but
  nothing is told.

[`Tag`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tag.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`TagTree::build`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagTree.html#method.build
[`TagTree::fingerprint`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagTree.html#method.fingerprint
[`Id`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/struct.Id.html
[`Tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tags.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagCounts`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagCounts.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`publish_tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/fn.publish_tags.html
[`owns`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/fn.owns.html
[`AttributeSystems`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributeSystems.html
