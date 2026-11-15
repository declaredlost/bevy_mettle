//! Abilities: marker types that define themselves, and the specs they're
//! granted by.

use bevy::prelude::*;
use bevy_mettle_core::registry::{AbilityId, AnyAbility, Keyed, Ref};

use crate::AbilityRegistry;
use crate::data::AbilityData;

/// An ability: a marker type that defines itself, by its derive's keys.
///
/// Derive it, each key on `#[ability(..)]` the [`AbilityData`] builder method
/// of the same name; add it to the app; and grant and activate it. Code names
/// abilities without a string, and a misspelled one doesn't compile.
///
/// ```ignore
/// #[derive(Ability)]
/// #[ability(behaviour::<CastFireball>, cost = Cost::of::<Mana>(20.0), cooldown = 2.0)]
/// pub struct Fireball;
///
/// app.add_ability::<Fireball>();
/// commands.entity(mage).grant_ability(Fireball.level(2.0)).activate_ability(Fireball);
/// ```
///
/// | Key | Meaning |
/// |---|---|
/// | `name = ".."` | Its key: its type's name in `snake_case`, unless it says |
/// | `define = function` | `fn(AbilityData) -> AbilityData`, defining all of it |
/// | `key`, `key = value`, `key(a, b)` | `.key()`, `.key(value)`, `.key(a, b)` |
///
/// Its builders make an [`AbilitySpec`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an ability",
    label = "not an ability",
    note = "derive it: `#[derive(Ability)]`"
)]
pub trait Ability: Keyed<Kind = AnyAbility> + Send + Sync + 'static {
    /// Its definition: `ability`, called by its key, filled in.
    fn define(ability: AbilityData) -> AbilityData;

    /// It, at level 1.
    #[must_use]
    fn spec(self) -> AbilitySpec
    where
        Self: Sized,
    {
        AbilitySpec::of::<Self>()
    }

    /// It, at `level`.
    #[must_use]
    fn level(self, level: f32) -> AbilitySpec
    where
        Self: Sized,
    {
        self.spec().level(level)
    }
}

pub use bevy_mettle_macros::Ability;

impl<A: Ability> From<A> for AbilitySpec {
    fn from(ability: A) -> Self {
        ability.spec()
    }
}

/// An ability about to be granted: which, and at what level.
///
/// Unreal's `GameplayAbilitySpec`, its ability and level. Made from an
/// [`Ability`] type (`Fireball.level(2.0)`); its ability is resolved to its
/// number as it's granted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilitySpec {
    ability: Ref<AnyAbility>,
    /// Its level: what its effects are applied at. 1 unless it says.
    pub level: f32,
}

impl AbilitySpec {
    /// `A`, at level 1.
    #[must_use]
    pub fn of<A: Ability>() -> Self {
        Self {
            ability: Ref::of::<A>(),
            level: 1.0,
        }
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// Its ability's number in `registry`: `None` if there's no such ability,
    /// or its type isn't added.
    #[must_use]
    pub fn ability_id(&self, registry: &AbilityRegistry) -> Option<AbilityId> {
        registry.find(self.ability)
    }

    /// Its ability, as named.
    pub(crate) fn named(&self) -> Ref<AnyAbility> {
        self.ability
    }
}

/// Adding the abilities code defines to an app, one or a tuple at once.
pub trait AbilityApp {
    /// Adds the ability `A`, defined by its type.
    fn add_ability<A: Ability>(&mut self) -> &mut Self;

    /// Adds each ability of `A`, a tuple of them: `add_abilities::<(Fireball,
    /// Blink)>()`.
    fn add_abilities<A: AbilityGroup>(&mut self) -> &mut Self;
}

impl AbilityApp for App {
    fn add_ability<A: Ability>(&mut self) -> &mut Self {
        self.add_abilities::<A>()
    }

    fn add_abilities<A: AbilityGroup>(&mut self) -> &mut Self {
        A::add_to(&mut self.world_mut().get_resource_or_init::<AbilityRegistry>());
        self
    }
}

/// Abilities added at once: an [`Ability`], or a tuple of up to 16.
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an ability, or a tuple of them",
    label = "not abilities"
)]
pub trait AbilityGroup {
    /// Adds each to `registry`.
    fn add_to(registry: &mut AbilityRegistry);
}

impl<A: Ability> AbilityGroup for A {
    fn add_to(registry: &mut AbilityRegistry) {
        registry.add::<A>();
    }
}

/// `AbilityGroup` for each tuple of them, up to 16.
macro_rules! abilities {
    ($first:ident $(, $rest:ident)*) => {
        impl<$first: AbilityGroup $(, $rest: AbilityGroup)*> AbilityGroup for ($first, $($rest,)*) {
            fn add_to(registry: &mut AbilityRegistry) {
                $first::add_to(registry);
                $($rest::add_to(registry);)*
            }
        }
        abilities!($($rest),*);
    };
    () => {};
}

abilities!(
    A0, A1, A2, A3, A4, A5, A6, A7, A8, A9, A10, A11, A12, A13, A14, A15
);
