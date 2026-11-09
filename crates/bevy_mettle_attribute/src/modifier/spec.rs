//! Modifiers named by type, made into [`Modifier`]s as they're put on an
//! entity: what code writes, as Bevy resolves a component's type as it's
//! inserted.

use std::any::TypeId;

use bevy::ecs::system::EntityCommands;
use bevy::prelude::*;
use bevy_mettle_core::Error;
use bevy_mettle_core::condition::Condition;
use bevy_mettle_core::tag::TagKey;

use super::{Modifier, Modifiers, Operation, Per};
use crate::Attribute;
use crate::registry::AttributeRegistry;

/// An attribute named by type, to be resolved to its number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Named {
    type_id: TypeId,
    name: &'static str,
}

impl Named {
    fn of<A: Attribute>() -> Self {
        Self {
            type_id: TypeId::of::<A>(),
            name: A::NAME,
        }
    }
}

/// A modifier as code writes it: its attributes named by type.
///
/// Made a [`Modifier`] by [`modifiers`](ModifierCommands::modifiers) as it's
/// put on an entity.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_attribute::prelude::*;
/// # #[derive(Attribute)] #[attribute(base = 10.0)] struct SpellPower;
/// # #[derive(Attribute)] #[attribute(base = 100.0)] struct Mana;
/// fn bless(
///     mut commands: Commands,
///     mage: Single<Entity, With<Base<SpellPower>>>,
/// ) {
///     commands.entity(*mage).modifiers([
///         ModifierSpec::flat::<SpellPower>(5.0),
///         ModifierSpec::flat::<SpellPower>(1.0).per::<Mana>(10.0),
///     ]);
/// }
/// # App::new().add_systems(Update, bless);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ModifierSpec {
    attribute: Named,
    op: Operation,
    value: f32,
    condition: Option<Condition>,
    per: Option<(Named, f32)>,
    local: bool,
}

impl ModifierSpec {
    /// `op` of `value` to `A`, everywhere and always.
    #[must_use]
    pub fn new<A: Attribute>(op: Operation, value: f32) -> Self {
        Self {
            attribute: Named::of::<A>(),
            op,
            value,
            condition: None,
            per: None,
            local: false,
        }
    }

    /// Replaces `A`'s base.
    #[must_use]
    pub fn base<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Base, value)
    }

    /// Adds `value` to `A`'s base.
    #[must_use]
    pub fn flat<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Flat, value)
    }

    /// `A` by a fraction (`0.2` for +20%), summed with the other additive
    /// ones.
    #[must_use]
    pub fn additive<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Additive, value)
    }

    /// `A` by a fraction (`0.2` for +20%), multiplied in on its own.
    #[must_use]
    pub fn multiplicative<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Multiplicative, value)
    }

    /// `A` divided by one more than a fraction, summed with the other dividing
    /// ones.
    #[must_use]
    pub fn divide<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Divide, value)
    }

    /// Adds `value` to `A` after everything else.
    #[must_use]
    pub fn unscaled<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Unscaled, value)
    }

    /// `A` is `value`, whatever else there is.
    #[must_use]
    pub fn overriding<A: Attribute>(value: f32) -> Self {
        Self::new::<A>(Operation::Override, value)
    }

    /// Only counting as `condition` says, and as much: added to any other
    /// condition it has, all of which have to hold.
    #[must_use]
    pub fn condition(mut self, condition: Condition) -> Self {
        self.condition = Some(match self.condition.take() {
            Some(before) => before.and(condition),
            None => condition,
        });
        self
    }

    /// Only counting where the read has `tag`, or one under it.
    #[must_use]
    pub fn read_tagged(self, tag: impl Into<TagKey>) -> Self {
        self.condition(Condition::read_tagged(tag))
    }

    /// Only counting while the owner owns `tag`, or one under it.
    #[must_use]
    pub fn owner_has(self, tag: impl Into<TagKey>) -> Self {
        self.condition(Condition::owner_has(tag))
    }

    /// Counting once for each whole `every` of `A` (more than 0).
    #[must_use]
    pub fn per<A: Attribute>(mut self, every: f32) -> Self {
        self.per = Some((Named::of::<A>(), every));
        self
    }

    /// Only counting for reads naming its source.
    #[must_use]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }

    /// The modifier it is in the app `attributes` are: its attributes by
    /// number.
    ///
    /// # Errors
    ///
    /// If an attribute it names isn't added.
    pub fn resolve(&self, attributes: &AttributeRegistry) -> Result<Modifier, Error> {
        let id = |named: Named| {
            attributes
                .id_of_type(named.type_id)
                .ok_or_else(|| Error::Invalid {
                    what: format!("a modifier of `{}`, which isn't added", named.name),
                    within: None,
                })
        };
        let per = match self.per {
            Some((named, every)) => Some(Per {
                attribute: id(named)?,
                every,
            }),
            None => None,
        };
        Ok(Modifier {
            attribute: id(self.attribute)?,
            op: self.op,
            value: self.value,
            condition: self.condition.clone(),
            per,
            local: self.local,
        })
    }
}

/// Resolves `specs` in `world`'s app: those naming an attribute that isn't
/// added skipped, with a warning.
fn resolve_all(world: &World, specs: &[ModifierSpec]) -> Modifiers {
    let Some(attributes) = world.get_resource::<AttributeRegistry>() else {
        warn!("modifiers given in a world without attributes: none put on");
        return Modifiers::default();
    };
    specs
        .iter()
        .filter_map(|spec| {
            spec.resolve(attributes)
                .inspect_err(|error| warn!("{error}: skipped"))
                .ok()
        })
        .collect()
}

/// Putting modifiers named by type on an entity: its [`Modifiers`], replaced.
pub trait ModifierCommands {
    /// Gives the entity `specs` as its [`Modifiers`], each made a [`Modifier`]
    /// in this app: as Bevy resolves a component's type as it's inserted.
    fn modifiers(&mut self, specs: impl IntoIterator<Item = ModifierSpec>) -> &mut Self;
}

impl ModifierCommands for EntityCommands<'_> {
    fn modifiers(&mut self, specs: impl IntoIterator<Item = ModifierSpec>) -> &mut Self {
        let specs: Vec<ModifierSpec> = specs.into_iter().collect();
        self.queue(move |mut entity: EntityWorldMut| {
            entity.modifiers(specs);
        })
    }
}

impl ModifierCommands for EntityWorldMut<'_> {
    fn modifiers(&mut self, specs: impl IntoIterator<Item = ModifierSpec>) -> &mut Self {
        let specs: Vec<ModifierSpec> = specs.into_iter().collect();
        let modifiers = resolve_all(self.world(), &specs);
        self.insert(modifiers)
    }
}
