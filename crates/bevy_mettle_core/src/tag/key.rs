//! Tag keys: every tag's name interned once, process-wide, as Bevy interns
//! labels, with the key of the tag right above it: read without a lock.

use std::any::TypeId;
use std::cell::RefCell;
use std::fmt;
use std::sync::{LazyLock, OnceLock, PoisonError, RwLock};

use bevy_platform::collections::HashMap;
use bevy_reflect::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Tag;

/// A gameplay tag, interned: cheap to copy, compare and look up, and the same
/// whether code names it by type ([`Tag`]) or data by name.
///
/// Made from a tag type (`Stunned.into()`, [`Tag::key`]), or from a name
/// (`TagKey::from("state.stunned")`), interned the first time it's seen. Its
/// number is this process's, never sent: data and the network name a tag by
/// its name, or by its place in the [`TagRegistry`](super::TagRegistry).
///
/// Written to data as its name.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Reflect)]
#[reflect(opaque, Clone, Debug, PartialEq, Hash, Serialize, Deserialize)]
pub struct TagKey(u32);

/// How many tags a chunk of the table holds.
const CHUNK: usize = 4096;

/// How many chunks the table can have: room for 4M tags.
const CHUNKS: usize = 1024;

/// An interned tag: its name, and the key of the tag right above it.
struct Entry {
    name: &'static str,
    parent: TagKey,
}

/// Every interned tag but the root, by key: chunks made as they're needed,
/// each entry written once, as it's interned. Read without a lock: a key is
/// only handed out once its entry is written.
static ENTRIES: [OnceLock<Box<[OnceLock<Entry>]>>; CHUNKS] = [const { OnceLock::new() }; CHUNKS];

/// Every tag's key by name: what interning takes a lock on.
static NAMES: LazyLock<RwLock<HashMap<&'static str, TagKey>>> =
    LazyLock::new(|| RwLock::new(HashMap::from_iter([("", TagKey::ROOT)])));

/// The root's entry: no name, above nothing.
static ROOT: Entry = Entry {
    name: "",
    parent: TagKey::ROOT,
};

/// `key`'s entry.
fn entry(key: TagKey) -> &'static Entry {
    if key == TagKey::ROOT {
        return &ROOT;
    }
    let index = key.index();
    ENTRIES[index / CHUNK]
        .get()
        .and_then(|chunk| chunk[index % CHUNK].get())
        .expect("a key is handed out once its tag is interned")
}

/// `name`'s key, interning it and every tag above it if they're new: called
/// with the names locked for writing, so each is interned once.
///
/// # Panics
///
/// Past 4M tags.
fn intern(names: &mut HashMap<&'static str, TagKey>, name: &str) -> TagKey {
    if let Some(&key) = names.get(name) {
        return key;
    }
    let parent = match name.rsplit_once('.') {
        Some((parent, _)) => intern(names, parent),
        None => TagKey::ROOT,
    };
    let name: &'static str = Box::leak(name.into());
    // The root is 0; every other tag after it, in the order interned.
    let index = names.len();
    assert!(index < CHUNK * CHUNKS, "fewer than {} tags", CHUNK * CHUNKS);
    let chunk =
        ENTRIES[index / CHUNK].get_or_init(|| (0..CHUNK).map(|_| OnceLock::new()).collect());
    let _ = chunk[index % CHUNK].set(Entry { name, parent });
    let key = TagKey(u32::try_from(index).expect("fewer than 2^32 tags"));
    names.insert(name, key);
    key
}

thread_local! {
    /// The tag types whose keys are being made on this thread, outermost
    /// first: one asked for again is its own ancestor.
    static MAKING: RefCell<Vec<(TypeId, &'static str)>> = const { RefCell::new(Vec::new()) };
}

impl TagKey {
    /// The root every tag hangs from: no name, never owned.
    pub const ROOT: Self = Self(0);

    /// The tag called `name`: `"state.stunned"`, interned with every tag above
    /// it if it's new.
    #[must_use]
    pub fn named(name: &str) -> Self {
        if let Some(&key) = NAMES
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(name)
        {
            return key;
        }
        intern(
            &mut NAMES.write().unwrap_or_else(PoisonError::into_inner),
            name,
        )
    }

    /// The tag called `segment` right under `parent`: `stunned` under `state`.
    #[must_use]
    pub fn child(parent: Self, segment: &str) -> Self {
        if parent == Self::ROOT {
            Self::named(segment)
        } else {
            Self::named(&format!("{}.{segment}", parent.name()))
        }
    }

    /// `T`'s key, kept in `cell`: what `#[derive(Tag)]` calls.
    ///
    /// # Panics
    ///
    /// If `T` is its own ancestor: its parents loop back to it.
    pub fn of<T: Tag>(cell: &'static OnceLock<Self>) -> Self {
        if let Some(&key) = cell.get() {
            return key;
        }
        let type_id = TypeId::of::<T>();
        let name = std::any::type_name::<T>();
        MAKING.with_borrow_mut(|making| {
            if let Some(start) = making.iter().position(|(made, _)| *made == type_id) {
                let looped: Vec<&str> = making[start..].iter().map(|(_, name)| *name).collect();
                panic!(
                    "tag `{name}` is its own ancestor: {} back to `{name}`",
                    looped.join(" under ")
                );
            }
            making.push((type_id, name));
        });
        let key = Self::child(T::Parent::key(), T::SEGMENT);
        MAKING.with_borrow_mut(Vec::pop);
        *cell.get_or_init(|| key)
    }

    /// Its name: `"state.stunned"`. The root's is empty.
    #[must_use]
    pub fn name(self) -> &'static str {
        entry(self).name
    }

    /// The tag right above it: `state` for `state.stunned`. `None` at the top.
    #[must_use]
    pub fn parent(self) -> Option<Self> {
        let parent = entry(self).parent;
        (self != Self::ROOT && parent != Self::ROOT).then_some(parent)
    }

    /// It, then every tag above it, nearest first: `damage.elemental.fire`,
    /// `damage.elemental`, `damage`.
    pub fn lineage(self) -> impl Iterator<Item = Self> {
        std::iter::successors((self != Self::ROOT).then_some(self), |key| key.parent())
    }

    /// Whether it's `tag` or under it: `damage.elemental.fire` is within
    /// `damage.elemental` and `damage`, not `damage.fire`.
    #[must_use]
    pub fn is_within(self, tag: impl Into<Self>) -> bool {
        let tag = tag.into();
        self.lineage().any(|key| key == tag)
    }

    /// Whether its name is a path: words of `a` to `z`, `0` to `9` and `_`,
    /// joined by `.`.
    #[must_use]
    pub fn is_path(self) -> bool {
        is_path(self.name())
    }

    /// Its index: where it's kept in a set or a count of tags.
    #[must_use]
    pub(crate) fn index(self) -> usize {
        self.0 as usize
    }

    /// The key at `index`.
    pub(crate) fn at(index: u32) -> Self {
        Self(index)
    }
}

/// Whether `name` is a path: words of `a` to `z`, `0` to `9` and `_`, joined by
/// `.`.
fn is_path(name: &str) -> bool {
    name.split('.').all(|word| {
        !word.is_empty()
            && word
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    })
}

impl From<&str> for TagKey {
    fn from(name: &str) -> Self {
        Self::named(name)
    }
}

impl<T: Tag> From<T> for TagKey {
    fn from(_: T) -> Self {
        T::key()
    }
}

impl fmt::Debug for TagKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tag({:?})", self.name())
    }
}

impl fmt::Display for TagKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Serialize for TagKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.name().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TagKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<str>::deserialize(deserializer)?;
        Ok(Self::named(&name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_interned_with_every_tag_above_it() {
        let fire = TagKey::named("damage.elemental.fire");
        assert_eq!(fire, TagKey::from("damage.elemental.fire"));
        assert_eq!(fire.name(), "damage.elemental.fire");
        let lineage: Vec<&str> = fire.lineage().map(TagKey::name).collect();
        assert_eq!(
            lineage,
            ["damage.elemental.fire", "damage.elemental", "damage"]
        );
        assert!(fire.is_within("damage.elemental"));
        assert!(fire.is_within("damage"));
        assert!(!fire.is_within("damage.elem"));
        assert!(!TagKey::named("damage").is_within(fire));
        assert_eq!(TagKey::named("damage").parent(), None);
        assert_eq!(
            TagKey::child(TagKey::named("damage"), "physical"),
            TagKey::named("damage.physical")
        );
    }

    #[test]
    fn keys_are_read_while_others_are_interned() {
        std::thread::scope(|scope| {
            for thread in 0..4 {
                scope.spawn(move || {
                    for n in 0..500 {
                        let name = format!("race.t{thread}.n{n}");
                        let key = TagKey::named(&name);
                        assert_eq!(key.name(), name);
                        let parent = key.parent().expect("under its thread's tag");
                        assert_eq!(parent.name(), format!("race.t{thread}"));
                        assert!(key.is_within("race"));
                    }
                });
            }
        });
    }

    #[test]
    fn a_path_is_words_joined_by_dots() {
        assert!(is_path("delivery.attack.melee_2"));
        for bad in ["", "a..b", ".a", "a.", "Fire", "a-b"] {
            assert!(!is_path(bad), "{bad}");
        }
    }

    #[test]
    fn it_reads_and_writes_as_its_name() {
        serde_test::assert_tokens(
            &TagKey::named("state.stunned"),
            &[serde_test::Token::Str("state.stunned")],
        );
    }
}
