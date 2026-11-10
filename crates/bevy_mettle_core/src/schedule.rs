//! The schedule mettle's gameplay runs in, and the steps it takes in order.
//!
//! Every part adds its systems to [`Mettle`], in its step: effects tick,
//! attributes fold, what follows other actors' values settles, values are
//! published. [`MettleCorePlugin`] runs it in the fixed step.

use bevy_app::{App, FixedPostUpdate, Plugin};
use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::{InternedScheduleLabel, IntoScheduleConfigs, ScheduleLabel, SystemSet};
use bevy_ecs::world::World;

/// The schedule mettle's gameplay runs in: each part in its [`MettleSet`].
///
/// Run in the fixed step, `FixedPostUpdate` unless [`MettleCorePlugin`] says
/// otherwise: what gameplay did in a tick's `FixedUpdate` is ticked, folded
/// and published before the tick ends, and a rollback runs it again.
#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mettle;

/// The steps of [`Mettle`], run one after another, in this order.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MettleSet {
    /// Active effects shown, ticked, expired and executed.
    Effects,
    /// Attributes folded: bases synced, modifiers folded, values worked out,
    /// bases following their limits.
    Attributes,
    /// What follows other actors' values settles: an effect reading its
    /// caster's attribute live, folded again until nothing moves.
    Settle,
    /// Values published: `Current`s written, changes told.
    Publish,
}

/// The system set [`Mettle`] is run from, which game systems order
/// themselves by.
///
/// It's in the schedule `Mettle` is run in, `FixedPostUpdate` by default.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MettleSystems;

/// Runs [`Mettle`], with its steps in order, in a schedule of the app's.
///
/// Every part's plugin adds it if it isn't there yet; add it first to run
/// mettle elsewhere than `FixedPostUpdate`. Added again to run it in the same
/// schedule, it does nothing; to run it in another, it panics, saying so.
pub struct MettleCorePlugin {
    run_in: InternedScheduleLabel,
}

impl MettleCorePlugin {
    /// Runs mettle in `schedule`.
    pub fn new(schedule: impl ScheduleLabel) -> Self {
        Self {
            run_in: schedule.intern(),
        }
    }

    /// The schedule it runs mettle in.
    #[must_use]
    pub fn run_in(&self) -> InternedScheduleLabel {
        self.run_in
    }
}

impl Default for MettleCorePlugin {
    /// Runs mettle in `FixedPostUpdate`, at the end of each fixed step.
    fn default() -> Self {
        Self::new(FixedPostUpdate)
    }
}

/// The schedule [`Mettle`] runs in, as [`MettleCorePlugin`] set it.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
struct RunIn(InternedScheduleLabel);

impl Plugin for MettleCorePlugin {
    fn is_unique(&self) -> bool {
        // Added again for the same schedule, it's nothing; for another, it
        // says so, as Bevy's own panic wouldn't.
        false
    }

    fn build(&self, app: &mut App) {
        if let Some(RunIn(running)) = app.world().get_resource::<RunIn>().copied() {
            assert!(
                running == self.run_in,
                "mettle already runs in `{running:?}`, so it can't run in `{:?}` too: add \
                 `MettleCorePlugin::new(..)` before any of mettle's plugins (each adds the \
                 default, `FixedPostUpdate`, if it isn't there), or set it on `MettlePlugins`",
                self.run_in
            );
            return;
        }
        app.insert_resource(RunIn(self.run_in))
            .init_schedule(Mettle)
            .configure_sets(
                Mettle,
                (
                    MettleSet::Effects,
                    MettleSet::Attributes,
                    MettleSet::Settle,
                    MettleSet::Publish,
                )
                    .chain(),
            )
            .add_systems(self.run_in, run.in_set(MettleSystems));
    }
}

/// Adds [`MettleCorePlugin`] to `app` if it isn't there yet, as parts
/// do.
pub fn ensure(app: &mut App) {
    if !app.world().contains_resource::<RunIn>() {
        app.add_plugins(MettleCorePlugin::default());
    }
}

/// Runs [`Mettle`].
fn run(world: &mut World) {
    world.run_schedule(Mettle);
}
