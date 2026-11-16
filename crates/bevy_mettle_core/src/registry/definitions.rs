//! The types of one kind that define themselves, and their definitions.

use std::any::TypeId;
use std::fmt;
use std::sync::Arc;

use super::{Keyed, Kind};

/// The types of one kind that define themselves, each with its key.
///
/// Of kind `K`, each with how it fills in its definition, a `D`.
///
/// What each self-defining kind's registry keeps (effects, abilities,
/// curves), as an app adds them: [`definitions`](Self::definitions) runs each
/// as content is checked, so a hot patch of a `define` is picked up on the
/// next check. Cheap to clone.
pub struct Definitions<K: Kind, D> {
    types: Arc<Vec<Defined<D>>>,
    kind: std::marker::PhantomData<K>,
}

struct Defined<D> {
    type_id: TypeId,
    type_name: &'static str,
    key: &'static str,
    define: fn(D) -> D,
}

impl<D> Clone for Defined<D> {
    fn clone(&self) -> Self {
        Self { ..*self }
    }
}

impl<K: Kind, D> Clone for Definitions<K, D> {
    fn clone(&self) -> Self {
        Self {
            types: Arc::clone(&self.types),
            kind: std::marker::PhantomData,
        }
    }
}

impl<K: Kind, D> Default for Definitions<K, D> {
    fn default() -> Self {
        Self {
            types: Arc::default(),
            kind: std::marker::PhantomData,
        }
    }
}

/// As their keys.
impl<K: Kind, D> fmt::Debug for Definitions<K, D> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.types.iter().map(|defined| defined.key))
            .finish()
    }
}

impl<K: Kind, D> Definitions<K, D> {
    /// Adds `T`, defined by `define`: nothing if it's in already.
    ///
    /// # Panics
    ///
    /// If another type has `T`'s key: content couldn't tell them apart.
    pub fn add<T: Keyed<Kind = K> + 'static>(&mut self, define: fn(D) -> D) {
        let type_id = TypeId::of::<T>();
        if self.types.iter().any(|defined| defined.type_id == type_id) {
            return;
        }
        if let Some(other) = self.types.iter().find(|defined| defined.key == T::KEY) {
            panic!(
                "`{}` and `{}` are both {}s keyed `{}`",
                other.type_name,
                std::any::type_name::<T>(),
                K::NAME,
                T::KEY,
            );
        }
        Arc::make_mut(&mut self.types).push(Defined {
            type_id,
            type_name: std::any::type_name::<T>(),
            key: T::KEY,
            define,
        });
    }

    /// Each type's key, with its type, in the order they were added.
    pub fn keys(&self) -> impl Iterator<Item = (TypeId, &'static str)> + '_ {
        self.types
            .iter()
            .map(|defined| (defined.type_id, defined.key))
    }

    /// Whether a type is keyed `key`.
    #[must_use]
    pub fn defines(&self, key: &str) -> bool {
        self.types.iter().any(|defined| defined.key == key)
    }

    /// Each type's definition, as its `define` fills in `start(key)`: through
    /// the hot patch's jump table with the `hotpatching` feature, so a patched
    /// `define` is the one run.
    pub fn definitions(&self, start: impl Fn(&'static str) -> D) -> Vec<D> {
        self.types
            .iter()
            .map(|defined| run(defined.define, start(defined.key)))
            .collect()
    }

    /// How many types there are.
    #[must_use]
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// Whether there are none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}

#[cfg(feature = "hotpatching")]
fn run<D>(define: fn(D) -> D, start: D) -> D {
    let mut start = Some(start);
    bevy_app::hotpatch::call(move || define(start.take().expect("called once")))
}

#[cfg(not(feature = "hotpatching"))]
fn run<D>(define: fn(D) -> D, start: D) -> D {
    define(start)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Kind;

    #[derive(Kind)]
    enum AnyThing {}

    struct Sword;
    impl Keyed for Sword {
        type Kind = AnyThing;

        const KEY: &'static str = "sword";
    }

    struct Blade;
    impl Keyed for Blade {
        type Kind = AnyThing;

        const KEY: &'static str = "sword";
    }

    #[test]
    fn each_type_fills_in_its_definition() {
        let mut definitions = Definitions::<AnyThing, String>::default();
        definitions.add::<Sword>(|start| start + ", sharp");
        definitions.add::<Sword>(|start| start + ", blunt");
        assert_eq!(definitions.definitions(str::to_owned), ["sword, sharp"]);
        assert!(definitions.defines("sword"));
    }

    #[test]
    #[should_panic(expected = "are both things keyed `sword`")]
    fn two_types_cant_share_a_key() {
        let mut definitions = Definitions::<AnyThing, String>::default();
        definitions.add::<Sword>(|start| start);
        definitions.add::<Blade>(|start| start);
    }
}
