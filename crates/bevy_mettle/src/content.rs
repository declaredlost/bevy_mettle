use bevy::ecs::change_detection::Tick;
use bevy::prelude::*;
#[cfg(feature = "ability")]
use bevy_mettle_ability::behaviour::Behaviours;
#[cfg(feature = "ability")]
use bevy_mettle_ability::data::{Ability, Grant};
#[cfg(feature = "ability")]
use bevy_mettle_ability::{AbilityChecks, AbilityRegistry};
#[cfg(feature = "effect")]
use bevy_mettle_attribute::registry::AttributeRegistry;
use bevy_mettle_core::curve::{CurveTable, LevelCurve};
use bevy_mettle_core::tag::{Tag, TagTree};
use bevy_mettle_core::{MettleError, MettleErrors};
#[cfg(feature = "effect")]
use bevy_mettle_effect::code::EffectCode;
#[cfg(feature = "effect")]
use bevy_mettle_effect::data::Effect;
#[cfg(feature = "effect")]
use bevy_mettle_effect::{EffectChecks, EffectRegistry};
use serde::{Deserialize, Serialize};

use crate::MettleSystems;

/// The game's content, all of it: tags, curves, effects, abilities and what
/// effects grant.
///
/// Mettle doesn't load anything: the game reads its files however it likes
/// and inserts this as a resource, as the app is built or again on a reload. As
/// it changes, [`MettleContentPlugin`] checks all of it at once, against the
/// attributes, behaviours and effect code the app added: if it all holds,
/// the tag tree, the curves and the registries are replaced and
/// [`ContentLoaded`] is triggered; if not, the content before stays, each
/// error is logged, and [`ContentRejected`] is triggered with them.
///
/// ```ron
/// (
///     tags: [(id: "cooldown"), (id: "cooldown.fireball")],
///     effects: [(id: "fireball.cooldown", duration: For(Value(2)), grants: ["cooldown.fireball"])],
/// )
/// ```
#[derive(Resource, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MettleContent {
    /// The gameplay tags there are.
    pub tags: Vec<Tag>,
    /// The curves effects scale by.
    pub curves: Vec<LevelCurve>,
    /// The effects there are.
    #[cfg(feature = "effect")]
    pub effects: Vec<Effect>,
    /// The abilities there are.
    #[cfg(feature = "ability")]
    pub abilities: Vec<Ability>,
    /// The abilities each effect grants while it lasts.
    #[cfg(feature = "ability")]
    pub grants: Vec<Grant>,
}

/// Content checked, ready to be loaded.
struct Checked {
    tags: TagTree,
    curves: CurveTable,
    #[cfg(feature = "effect")]
    effects: EffectRegistry,
    #[cfg(feature = "ability")]
    abilities: AbilityRegistry,
}

impl MettleContent {
    /// Checks all of it against what `world` added: the attributes, the
    /// behaviours and the effect code. What [`MettleContentPlugin`] checks it
    /// by, for an answer at once: a mod before it's loaded, a test.
    ///
    /// # Errors
    ///
    /// Everything wrong, at once. Effects are checked once the tags and the
    /// curves hold, and abilities once the effects do.
    pub fn check(&self, world: &World) -> Result<(), MettleErrors> {
        self.checked(world).map(|_| ())
    }

    fn checked(&self, world: &World) -> Result<Checked, MettleErrors> {
        let mut errors = Vec::new();
        let tags = held(TagTree::build(&self.tags), &mut errors);
        let curves = held(CurveTable::build(&self.curves), &mut errors);
        #[cfg(feature = "effect")]
        let effects = match (&tags, &curves) {
            (Some(tags), Some(curves)) => {
                let (no_attributes, no_code) =
                    (AttributeRegistry::default(), EffectCode::default());
                held(
                    EffectRegistry::check(
                        &self.effects,
                        EffectChecks {
                            tags,
                            curves,
                            attributes: world.get_resource().unwrap_or(&no_attributes),
                            code: world.get_resource().unwrap_or(&no_code),
                        },
                    ),
                    &mut errors,
                )
            }
            _ => None,
        };
        #[cfg(feature = "ability")]
        let abilities = match (&tags, &effects) {
            (Some(tags), Some(effects)) => {
                let no_behaviours = Behaviours::default();
                held(
                    AbilityRegistry::check(
                        &self.abilities,
                        &self.grants,
                        AbilityChecks {
                            tags,
                            effects,
                            behaviours: world.get_resource().unwrap_or(&no_behaviours),
                        },
                    ),
                    &mut errors,
                )
            }
            _ => None,
        };
        #[cfg(not(feature = "effect"))]
        let _ = world;
        match (errors.is_empty(), tags, curves) {
            (true, Some(tags), Some(curves)) => Ok(Checked {
                tags,
                curves,
                #[cfg(feature = "effect")]
                effects: effects.expect("checked"),
                #[cfg(feature = "ability")]
                abilities: abilities.expect("checked"),
            }),
            _ => Err(MettleErrors(errors)),
        }
    }
}

/// What `result` built, its errors kept in `errors`.
fn held<T>(result: Result<T, MettleErrors>, errors: &mut Vec<MettleError>) -> Option<T> {
    result.map_err(|wrong| errors.extend(wrong.0)).ok()
}

/// Triggered as [`MettleContent`] has been checked and loaded.
#[derive(Event, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentLoaded;

/// Triggered as [`MettleContent`] was found wrong: the content before stays.
#[derive(Event, Debug, Clone, PartialEq)]
pub struct ContentRejected {
    /// Everything wrong with it.
    pub errors: MettleErrors,
}

/// Loads [`MettleContent`] as it changes, before any part runs: checked, then
/// its tag tree, curves and registries replaced, or rejected.
///
/// Inserted as the app is built, it's loaded in `PreStartup`, for `Startup`
/// systems to grant and apply what it defines; inserted or changed later, in
/// the next `PreUpdate`.
///
/// In [`MettlePlugins`](crate::MettlePlugins); add it yourself if you add the
/// parts' plugins one by one.
#[derive(Default)]
pub struct MettleContentPlugin;

impl Plugin for MettleContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Loaded>()
            .add_systems(PreStartup, load.run_if(changed))
            .add_systems(PreUpdate, load.run_if(changed).before(MettleSystems));
    }
}

/// The change to [`MettleContent`] last loaded or rejected: whichever
/// schedule saw it first, once.
#[derive(Resource, Default)]
struct Loaded(Option<Tick>);

/// Whether the content changed since it was last loaded or rejected: so
/// [`load`], which needs the whole world, runs only then.
fn changed(content: Option<Res<MettleContent>>, loaded: Res<Loaded>) -> bool {
    content.is_some_and(|content| loaded.0 != Some(content.last_changed()))
}

/// Checks the content as it changed, and loads it or rejects it.
fn load(world: &mut World) {
    let changed = world.resource_ref::<MettleContent>().last_changed();
    world.resource_mut::<Loaded>().0 = Some(changed);
    let checked = world.resource::<MettleContent>().checked(world);
    match checked {
        Ok(checked) => {
            world.insert_resource(checked.tags);
            world.insert_resource(checked.curves);
            #[cfg(feature = "effect")]
            world.insert_resource(checked.effects);
            #[cfg(feature = "ability")]
            world.insert_resource(checked.abilities);
            world.trigger(ContentLoaded);
        }
        Err(errors) => {
            for error in &errors.0 {
                error!("mettle content rejected: {error}");
            }
            world.trigger(ContentRejected { errors });
        }
    }
}
