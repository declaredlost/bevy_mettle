//! Abilities code names: marker types, their content's names on them, and the
//! specs they're granted by.

use std::any::TypeId;

use bevy::prelude::*;
use bevy_mettle_core::registry::AbilityId;

use crate::AbilityRegistry;

/// An ability code names: a marker type, its content's name on it.
///
/// Derive it, add it to the app, and grant or activate it: code names
/// abilities without a string, and a misspelled one doesn't compile.
///
/// ```ignore
/// #[derive(Ability)]
/// pub struct Fireball;
///
/// app.add_ability::<Fireball>();
/// commands.entity(mage).grant_ability(Fireball.level(2.0)).activate_ability(Fireball);
/// ```
///
/// [`add_ability`](AbilityApp::add_ability) makes content without it fail
/// its check, so its name is checked once, as content loads. Its builders make
/// an [`AbilitySpec`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an ability",
    label = "not an ability",
    note = "derive it: `#[derive(Ability)]`"
)]
pub trait Ability: Send + Sync + 'static {
    /// The name content defines it by: `"fireball"`.
    const NAME: &'static str;

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

/// Which ability: by type, as code names it, or by number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Type { type_id: TypeId, name: &'static str },
    Number(AbilityId),
}

impl Which {
    fn resolve(self, registry: &AbilityRegistry) -> Option<AbilityId> {
        match self {
            Self::Type { type_id, .. } => registry.id_of_type(type_id),
            Self::Number(id) => registry.get(id).map(|_| id),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Type { name, .. } => name.to_owned(),
            Self::Number(id) => format!("{id:?}"),
        }
    }
}

/// An ability about to be granted: which, and at what level.
///
/// Unreal's `GameplayAbilitySpec`, its ability and level. Made from an
/// [`Ability`] type (`Fireball.level(2.0)`), or from a number
/// ([`from_id`](Self::from_id)); its ability is resolved to its number as
/// it's granted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AbilitySpec {
    ability: Which,
    /// Its level: what its effects are applied at. 1 unless it says.
    pub level: f32,
}

impl AbilitySpec {
    /// `A`, at level 1.
    #[must_use]
    pub fn of<A: Ability>() -> Self {
        Self {
            ability: Which::Type {
                type_id: TypeId::of::<A>(),
                name: A::NAME,
            },
            level: 1.0,
        }
    }

    /// The ability numbered `ability`, at level 1: for one code doesn't name
    /// by type, found by its number.
    #[must_use]
    pub fn from_id(ability: AbilityId) -> Self {
        Self {
            ability: Which::Number(ability),
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
        self.ability.resolve(registry)
    }

    /// What its ability is called, for a warning.
    pub(crate) fn label(&self) -> String {
        self.ability.label()
    }
}

/// What names an ability to activate: an [`Ability`] type, or its number.
pub trait IntoAbilityId: Send + Sync + 'static {
    /// Its number in `registry`: `None` if there's no such ability, or its type
    /// isn't added.
    fn ability_id(&self, registry: &AbilityRegistry) -> Option<AbilityId>;

    /// What it's called, for a warning.
    fn label(&self) -> String;
}

impl<A: Ability> IntoAbilityId for A {
    fn ability_id(&self, registry: &AbilityRegistry) -> Option<AbilityId> {
        registry.id_of::<A>()
    }

    fn label(&self) -> String {
        A::NAME.to_owned()
    }
}

impl IntoAbilityId for AbilityId {
    fn ability_id(&self, registry: &AbilityRegistry) -> Option<AbilityId> {
        registry.get(*self).map(|_| *self)
    }

    fn label(&self) -> String {
        format!("{self:?}")
    }
}

/// Adding the abilities code names to an app, which content has to define.
pub trait AbilityApp {
    /// Adds the ability `A` code names: content has to define it, or it's
    /// rejected.
    fn add_ability<A: Ability>(&mut self) -> &mut Self;
}

impl AbilityApp for App {
    fn add_ability<A: Ability>(&mut self) -> &mut Self {
        self.world_mut()
            .get_resource_or_init::<AbilityRegistry>()
            .name_type::<A>();
        self
    }
}
