# Gameplay tags

A gameplay tag is a label in a tree: `damage.elemental.fire` sits under
`damage.elemental`, which sits under `damage`. Everything in mettle is
labelled with tags. The mage's fireball is tagged `ability.spell.fire`, the
burn it leaves is `effect.affliction.burning`, and while it burns the mage
owns `state.burning`.

What has a tag has every tag above it. A rule written for `damage.elemental`
holds for a fire hit, and a cure for `effect.affliction` takes off burning
and poison alike. This is how Unreal's gameplay tags work too.

## Tags in code

Code names a tag by its type. Derive [`Tag`] on a unit struct, under its
parent:

```rust
# use bevy_mettle::prelude::*;
#[derive(Tag)]
struct State;                    // "state"

#[derive(Tag)]
#[tag(parent = State)]
struct Burning;                  // "state.burning"

#[derive(Tag)]
#[tag(parent = State, name = "wet")]
struct Soaked;                   // "state.wet"

let mut owned = OwnedTags::default();
owned.add(Burning);
assert!(owned.contains(Burning));
assert!(owned.contains(State)); // what's above it too
```

A tag's name is its type's in `snake_case` under its parent's, unless
`name` says otherwise. A tag with no `parent` is at the top. Its parent is
[`Root`], which every tag hangs from and nothing ever owns.

Naming a tag by type means a misspelled one doesn't compile:
`owned.contains(Burnign)` is an error, where `"state.burnign"` would quietly
never match. The parent is a type too, so the tree in code can't be
misspelled either.

Tags code names are added to the app, as attributes are:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
# #[derive(Tag)] struct State;
# #[derive(Tag)] #[tag(parent = State)] struct Burning;
App::new().add_tag::<Burning>(); // and `state`, above it
```

They join the registry content is checked against, whether the content
defines them or not, so data naming them is checked against code.

## Keys

Wherever mettle takes a tag, it takes anything that becomes a [`TagKey`]:
a tag type (`Burning`), or a name (`"state.burning"`). A key is the tag
interned: a number, cheap to copy and compare, the same whether code named
it by type or data by name. That number is this process's alone. What's
sent over the network is the tag's number in the registry, a [`TagId`], which
is the same on a server and its clients: see [The tag registry](#the-tag-registry).

A key knows where it sits in the tree:

```rust
# use bevy_mettle::prelude::*;
#[derive(Tag)]
struct Damage;
#[derive(Tag)]
#[tag(parent = Damage)]
struct Elemental;
#[derive(Tag)]
#[tag(parent = Elemental)]
struct Fire;

let fire = TagKey::from(Fire);
assert_eq!(fire, TagKey::named("damage.elemental.fire"));
assert_eq!(fire.parent(), Some(TagKey::from(Elemental)));
assert!(fire.is_within(Damage));

let lineage: Vec<&str> = fire.lineage().map(TagKey::name).collect();
assert_eq!(lineage, ["damage.elemental.fire", "damage.elemental", "damage"]);
```

## The tag registry

Tags that only data names needn't be types. Content lists them by name,
and a mod can add its own. In data, a tag is written as its name:
`"damage.elemental.fire"`.

In a game, they go in [`MettleContent`]'s `tags`, beside those code adds,
and mettle checks them as the content is loaded, keeping them as a
resource, `Res<TagRegistry>`. The tags code adds are kept in the registry
too, so a reload doesn't lose them. [`TagRegistry::build`] is that step on
its own: it checks the definitions and gives you the registry everything
else is checked against:

```rust
# use bevy_mettle::prelude::*;
let definitions = ["damage", "damage.elemental", "damage.elemental.fire"].map(TagKey::named);
let tags = TagRegistry::build(definitions).unwrap();

assert!(tags.contains("damage.elemental.fire"));
assert!(!tags.contains("damage.poison"));
```

The registry numbers its tags: each gets a [`TagId`], in order of name. The
same tags are numbered the same wherever they're checked, so a server and
its clients agree on every number. `id` and `key` go between the two:

```rust
# use bevy_mettle::prelude::*;
# let definitions = ["damage", "damage.elemental", "damage.elemental.fire"].map(TagKey::named);
# let tags = TagRegistry::build(definitions).unwrap();
let fire: TagId = tags.id("damage.elemental.fire").unwrap();
assert_eq!(tags.key(fire), Some(TagKey::named("damage.elemental.fire")));

// A reload keeps the numbers tags had, and numbers new ones after them.
let reloaded = tags
    .rebuild(["damage", "damage.elemental", "damage.elemental.fire", "damage.poison"].map(TagKey::named))
    .unwrap();
assert_eq!(reloaded.id("damage.elemental.fire"), Some(fire));
```

Mettle rebuilds the registry this way each time content is loaded again, so
what holds a tag's number still names the same tag.

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
let definitions = [TagKey::named("damage"), TagKey::named("damage.elemental.fire")];
let errors = TagRegistry::build(definitions).unwrap_err();
assert_eq!(
    errors.to_string(),
    "there's no tag called `damage.elemental` (in `damage.elemental.fire`)"
);
```

Mettle doesn't read files. Your game loads them however it likes, and hands
the definitions over: see [Loading data](loading-data.md).

## Sets of tags

[`Tags`] is a set: what an effect, an ability or a hit is labelled with. It
holds every tag above the ones you give it, so asking it for `effect` finds
`effect.affliction.burning`:

```rust
# use bevy_mettle::prelude::*;
let burning: Tags = ["effect.affliction.burning"].into_iter().collect();

assert!(burning.contains("effect.affliction"));
assert!(burning.contains("effect"));
// Written back as given, without what's above.
assert_eq!(burning.written().count(), 1);
```

In data, a set is written as a list of the tags as given, each by its name.

## Owned tags

An actor's states are tags it owns: [`OwnedTags`], a component. The game
marks the mage `state.casting` while it casts. Effects tag their owner too: while
the mage burns, it owns `state.burning`. Every actor with an attribute has
`OwnedTags`.

Owned tags are counted. Each source adds one and later takes its one away.
If two effects both give it `immune.fire`, the mage stays immune until both
are gone:

```rust
# use bevy_mettle::prelude::*;
#[derive(Tag)]
struct Immune;
#[derive(Tag)]
#[tag(parent = Immune, name = "fire")]
struct ImmuneToFire;

let mut owned = OwnedTags::default();
owned.add(ImmuneToFire); // A potion.
owned.add(ImmuneToFire); // A ring.

owned.remove(ImmuneToFire); // The potion wears off.
assert!(owned.contains(ImmuneToFire));
assert!(owned.contains(Immune));

owned.remove(ImmuneToFire); // The ring comes off.
assert!(!owned.contains(ImmuneToFire));
```

Removing a tag only takes away what was added as that tag. Owning
`immune.fire` means owning `immune`, but `owned.remove(Immune)` does
nothing: no source added `immune` itself. `OwnedTags` reads as a
[`TagCounts`], which holds the counts: `count(tag)` says how many sources
hold a tag, and `iter()` lists them. Counts are kept by key, so asking is an
index, not a comparison of names.

### Told as they come and go

When an actor comes to own a tag, [`TagAdded`] is triggered on it. When it
stops, [`TagRemoved`] is. Each tag above counts too: the first fire
immune state tells both `immune.fire` and `immune`. Observe them on the actor,
or globally:

```rust
# use bevy::prelude::*;
# use bevy_mettle::prelude::*;
#[derive(Resource, Default)]
struct Told(Vec<String>);

let mut app = App::new();
// Mettle runs in `Update` here, so one update tells them.
app.add_plugins((MinimalPlugins, MettlePlugins.set(MettleCorePlugin::new(Update))))
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
    .add("state.burning");
app.update();

assert_eq!(app.world().resource::<Told>().0, ["+state.burning", "+state"]);
```

They're told as the [`publish_tags`] system runs, as attributes are folded
([`AttributeFold`]), before values are worked out. Changes are settled
first: a tag added and taken away again before then isn't told at all.

### Asking from a behavior tree

[`owns`] is a guard: a read-only system that takes an agent and a tag key,
and says whether the agent owns the tag or one under it. It has the shape
`bevy_arbor` takes for a decorator, so a tree can wait on `state.stunned`.

## Checking a client numbers tags as its server does

A client and its server have to agree on the registry: tags travel as their
numbers, so a tag numbered differently would be read as another.
[`TagRegistry::fingerprint`], as every registry's, is a hash of the tags with
their numbers. Built from the same tags, in whatever order they were defined,
registries hash the same; a registry rebuilt by a reload keeps old numbers, so
two hash the same only if they were loaded the same way. Over lightyear,
mettle compares them for you as a client connects:
[Lightyear](lightyear.md#checking-content).

## Pitfalls

- **Names aren't checked where you write them.** `"stat.burning"` compiles
  wherever a tag is taken. Name tags code uses by type; a name is caught only
  when data naming it is checked against the registry.
- **Facts are tags too.** [Conditions](conditions.md) name the numbers a read
  supplies by tags, such as `fact.flown`. Define them as tags, or
  conditions naming them fail their checks.
- **A tag can't be under itself.** `#[tag(parent = ..)]` loops compile, but
  panic with both names the first time either tag is used.
- **No events without the attribute plugin.** `publish_tags` runs in
  `AttributeFold`. Without `AttributePlugin`, owned tags still count, but
  nothing is told.

[`Tag`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/trait.Tag.html
[`Root`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/enum.Root.html
[`TagKey`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagKey.html
[`MettleContent`]: https://docs.rs/bevy_mettle/latest/bevy_mettle/struct.MettleContent.html
[`TagId`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/type.TagId.html
[`TagRegistry::build`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRegistry.html#method.build
[`TagRegistry::fingerprint`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/registry/struct.Registry.html#method.fingerprint
[`Tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.Tags.html
[`OwnedTags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.OwnedTags.html
[`TagCounts`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagCounts.html
[`TagAdded`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagAdded.html
[`TagRemoved`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/struct.TagRemoved.html
[`publish_tags`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/fn.publish_tags.html
[`owns`]: https://docs.rs/bevy_mettle_core/latest/bevy_mettle_core/tag/fn.owns.html
[`AttributeFold`]: https://docs.rs/bevy_mettle_attribute/latest/bevy_mettle_attribute/struct.AttributeFold.html
