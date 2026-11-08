//! Asking about abilities from an ordinary system: a UI, an AI.

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::engine;
use crate::events::Failure;

/// Asks about granted abilities, with nothing done: whether one could
/// activate, could pay, and how long its cooldown has left.
///
/// For a UI greying out what can't be cast, a cooldown sweep, or an AI
/// choosing what to cast. It reads the whole world, so it doesn't run
/// alongside systems that change abilities, effects or tags.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_ability::prelude::*;
/// fn buttons(
///     abilities: Query<Entity, With<GrantedAbility>>,
///     reader: AbilityReader,
/// ) {
///     for ability in &abilities {
///         let usable = reader.can_activate(ability).is_ok();
///         let cooling = reader.cooldown_remaining(ability);
///         let _ = (usable, cooling);
///     }
/// }
/// # App::new().add_systems(Update, buttons);
/// ```
#[derive(SystemParam)]
pub struct AbilityReader<'w> {
    world: &'w World,
}

impl AbilityReader<'_> {
    /// Whether the granted `ability` could activate now, and if not, why:
    /// every check activating it makes.
    ///
    /// # Errors
    ///
    /// Why it couldn't activate.
    pub fn can_activate(&self, ability: Entity) -> Result<(), Failure> {
        engine::can_activate(self.world, ability)
    }

    /// Whether the granted `ability`'s owner could pay its cost now.
    #[must_use]
    pub fn can_afford(&self, ability: Entity) -> bool {
        engine::can_afford(self.world, ability)
    }

    /// The seconds left on the granted `ability`'s cooldown, if it's cooling
    /// down, as [`can_activate`](Self::can_activate) weighs it; forever if
    /// nothing that cools it runs out.
    #[must_use]
    pub fn cooldown_remaining(&self, ability: Entity) -> Option<f32> {
        engine::cooldown_remaining(self.world, ability)
    }
}
