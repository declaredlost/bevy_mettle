//! Cues: what's shown of effects and abilities, for the game to play.
//!
//! A cue is a gameplay tag, `cue.burning`, `cue.hit.fire`. A *lasting* cue
//! holds while something lasts: an effect while it counts, an ability while
//! it's active, or one the game adds. Each is an entity of its own,
//! [`ActiveCue`] [`CueOf`] its actor, spawned as it starts and despawned as it
//! ends: the game observes `On<Add<ActiveCue>>` and `On<Remove<ActiveCue>>`,
//! or queries them, and hangs what it plays on them. A *one-shot* cue plays
//! at a moment, an effect executing or one the game executes: it triggers
//! [`CueExecuted`] on the actor.
//!
//! Over a network, the server's lasting cues ([`ServedCues`]) and its recent
//! one-shots ([`CueBursts`]) are replicated with the actor, to whoever sees
//! it: a client shows them next to its own, and plays a one-shot it predicted
//! once.

use std::collections::BTreeSet;

use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::ecs::query::SpawnDetails;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::net::NetRole;
use bevy_mettle_core::registry::{EffectId, TagId};
use bevy_mettle_core::tag::{TagKey, TagRegistry};
use serde::{Deserialize, Serialize};

use crate::context::{ContextDataRegistry, ContextReplica, EffectContext};
use crate::events::Delta;

/// A one-shot cue to play: its tag, and what it's played with.
///
/// Played with [`execute_cue`](CueCommands::execute_cue). What the game plays
/// carries what it needs as [context data](crate::context::ContextData):
/// a number, where it hit.
#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    /// Its tag: `cue.hit.fire`.
    pub tag: TagKey,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, and the game's own data.
    pub context: EffectContext,
    /// Whether it's this world's alone: a server doesn't replicate it.
    pub unreplicated: bool,
    /// What the effect playing it changed each attribute's base by.
    pub(crate) changes: Vec<Delta>,
}

impl Cue {
    /// The cue `tag`, at level 1, from no one.
    #[must_use]
    pub fn new(tag: impl Into<TagKey>) -> Self {
        Self {
            tag: tag.into(),
            level: 1.0,
            context: EffectContext::default(),
            unreplicated: false,
            changes: Vec::new(),
        }
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// Caused by `source`.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.context.source = Some(source);
        self
    }

    /// Played with `context`: who caused it, and the game's own data.
    #[must_use]
    pub fn context(mut self, context: EffectContext) -> Self {
        self.context = context;
        self
    }

    /// With `data`, the game's own, on its context: in place of any of its
    /// type.
    #[must_use]
    pub fn with<C: crate::context::ContextData>(mut self, data: C) -> Self {
        self.context = self.context.with(data);
        self
    }

    /// This world's alone: a server doesn't replicate it to clients.
    #[must_use]
    pub fn unreplicated(mut self) -> Self {
        self.unreplicated = true;
        self
    }
}

/// On an actor: what holds each of its lasting cues here, by count.
///
/// An effect counting, an ability active, the game. Those a server serves,
/// and those it doesn't replicate.
#[derive(Component, Debug, Clone, Default, PartialEq, Eq)]
#[require(ServedCues)]
pub struct CueSources {
    served: HashMap<TagKey, u32>,
    unreplicated: HashMap<TagKey, u32>,
}

impl CueSources {
    /// The counts of the sources a server serves, or of those it doesn't.
    fn counts(&mut self, unreplicated: bool) -> &mut HashMap<TagKey, u32> {
        if unreplicated {
            &mut self.unreplicated
        } else {
            &mut self.served
        }
    }

    /// One more source of `tag`.
    fn add(&mut self, tag: TagKey, unreplicated: bool) {
        *self.counts(unreplicated).entry(tag).or_default() += 1;
    }

    /// One less source of `tag`: whether there was one.
    pub(crate) fn remove(&mut self, tag: TagKey, unreplicated: bool) -> bool {
        let counts = self.counts(unreplicated);
        let Some(count) = counts.get_mut(&tag) else {
            return false;
        };
        *count -= 1;
        if *count == 0 {
            counts.remove(&tag);
        }
        true
    }

    /// The cues held, served or not.
    pub fn iter(&self) -> impl Iterator<Item = TagKey> {
        self.served.keys().chain(self.unreplicated.keys()).copied()
    }

    /// The cues held that a server serves.
    fn served(&self) -> BTreeSet<TagKey> {
        self.served.keys().copied().collect()
    }

    /// Whether a source a server serves holds `tag`.
    fn serves(&self, tag: TagKey) -> bool {
        self.served.contains_key(&tag)
    }
}

/// On an actor: the lasting cues its server holds on it, replicated.
///
/// Replicated to the clients that see it. By their numbers in the
/// [`TagRegistry`]: a cue the tree hasn't isn't served.
#[derive(
    Component,
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    Serialize,
    Deserialize
)]
pub struct ServedCues(pub BTreeSet<TagId>);

/// On a lasting cue's entity: which cue it is, while it plays.
///
/// Spawned as the cue starts, [`CueOf`] its actor, and despawned as it ends:
/// observe `On<Add<ActiveCue>>` and `On<Remove<ActiveCue>>`, or query it.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// fn burn(added: On<Add<ActiveCue>>, cues: Query<(&ActiveCue, &CueOf)>) {
///     let Ok((cue, of)) = cues.get(added.entity) else {
///         return;
///     };
///     if cue.tag.is_within("cue.burning") {
///         // Hang flames on `added.entity`, by `of.0`.
///     }
/// }
/// # App::new().add_observer(burn);
/// ```
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct ActiveCue {
    /// Its tag.
    pub tag: TagKey,
    /// Whether it was going already as this world came to see its actor: a
    /// client the actor came into view of, rather than one that saw it start.
    pub joined: bool,
    /// Whether it's this world's alone: held only by sources a server doesn't
    /// replicate.
    pub unreplicated: bool,
}

/// On a lasting cue's entity: the actor it plays on, as a Bevy relationship.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
#[relationship(relationship_target = ActiveCues)]
pub struct CueOf(pub Entity);

/// On an actor: its lasting cues' entities, kept by [`CueOf`]; despawned with
/// it.
#[derive(Component, Debug, Default)]
#[relationship_target(relationship = CueOf, linked_spawn)]
pub struct ActiveCues(Vec<Entity>);

impl ActiveCues {
    /// The cues' entities.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.0.iter().copied()
    }
}

/// A one-shot cue a server played, as it's replicated in [`CueBursts`].
///
/// Its tag by its number in the [`TagRegistry`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Burst {
    /// Its number, counting up on its actor.
    pub seq: u64,
    /// The server's elapsed seconds as it played: how old it is.
    pub at: f64,
    /// Its tag.
    pub tag: TagId,
    /// What it changed each attribute's base by.
    pub changes: Vec<Delta>,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, and context data.
    pub context: ContextReplica,
    /// What it was played under, if an activation.
    pub activation: Option<Activation>,
    /// The effect that played it, if one did.
    pub effect: Option<EffectId>,
}

/// On an actor: the one-shot cues its server played lately, replicated.
///
/// Replicated to the clients that see it, which play those they haven't.
#[derive(Component, Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[component(map_entities)]
pub struct CueBursts {
    /// The next one's number.
    pub next: u64,
    /// Those played in the last second, oldest first.
    pub recent: Vec<Burst>,
}

impl MapEntities for CueBursts {
    fn map_entities<M: EntityMapper>(&mut self, mapper: &mut M) {
        for burst in &mut self.recent {
            burst.context.map_entities(mapper);
        }
    }
}

/// How long a server keeps a one-shot for its clients, in seconds: what a
/// client coming to see an actor plays of what's just happened on it.
const KEPT: f64 = 0.5;

/// How long a client remembers a one-shot it predicted, for the server's
/// copy, in seconds: past what the server keeps it for, it won't come.
const REMEMBERED: f64 = 2.0 * KEPT;

/// On a client's actor: the next one-shot it hasn't played.
#[derive(Component, Debug, Clone, Copy, Default)]
pub(crate) struct Played(u64);

/// On a client: the one-shots it played under its own activations, so the
/// server's copies aren't played again; each as often as it was played, with
/// the client's seconds it was played at.
#[derive(Resource, Debug, Default)]
pub(crate) struct Predicted(HashMap<(Entity, Activation, TagKey), Vec<f64>>);

impl Predicted {
    /// Notes `tag` played on `actor` under `activation` at `at`.
    fn note(&mut self, actor: Entity, activation: Activation, tag: TagKey, at: f64) {
        self.0.entry((actor, activation, tag)).or_default().push(at);
    }

    /// Takes one play of `tag` on `actor` under `activation`: whether there
    /// was one, the server's copy of it.
    fn take(&mut self, actor: Entity, activation: Activation, tag: TagKey) -> bool {
        let key = (actor, activation, tag);
        let Some(plays) = self.0.get_mut(&key) else {
            return false;
        };
        plays.remove(0);
        if plays.is_empty() {
            self.0.remove(&key);
        }
        true
    }

    /// Forgets what was played before `before`: its copy won't come now.
    fn forget(&mut self, before: f64) {
        self.0.retain(|_, plays| {
            plays.retain(|at| *at >= before);
            !plays.is_empty()
        });
    }

    /// Forgets what was played under `activation`: the server rejected it,
    /// and plays none of it.
    pub(crate) fn forget_activation(&mut self, activation: Activation) {
        self.0.retain(|(_, under, _), _| *under != activation);
    }
}

/// Triggered on an actor as a one-shot cue plays on it, with what it dealt.
///
/// ```
/// # use bevy::prelude::*;
/// # use bevy_mettle_effect::prelude::*;
/// fn spark(played: On<CueExecuted>) {
///     if played.tag.is_within("cue.hit") {
///         let dealt = played.magnitude();
///         // A spark on `played.entity`, as big as `dealt`.
///         let _ = dealt;
///     }
/// }
/// # App::new().add_observer(spark);
/// ```
#[derive(EntityEvent, Debug, Clone, PartialEq)]
pub struct CueExecuted {
    /// The actor.
    pub entity: Entity,
    /// Its tag.
    pub tag: TagKey,
    /// What it changed each attribute's base by, in the order it first
    /// changed it.
    pub changes: Vec<Delta>,
    /// The level of what played it.
    pub level: f32,
    /// What played it carried: who caused it, and context data.
    pub context: EffectContext,
    /// What it was played under, if an activation.
    pub activation: Option<Activation>,
    /// The effect that played it, if one did.
    pub effect: Option<EffectId>,
}

impl CueExecuted {
    /// How much: what it dealt the first attribute it changed, the one it's
    /// about (a fireball's life). 0 if it changed none.
    #[must_use]
    pub fn magnitude(&self) -> f32 {
        self.changes.first().map_or(0.0, |change| change.delta)
    }
}

/// The system set cues are shown and played in, in `PostUpdate`.
///
/// Lasting cues are shown in it, and one-shots from the server played.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CueSystems;

/// Playing cues on actors, as commands: one-shots, and lasting cues held.
pub trait CueCommands {
    /// Plays the one-shot `cue` on this actor.
    fn execute_cue(&mut self, cue: Cue) -> &mut Self;

    /// Holds the lasting cue `tag` on this actor once more, served to clients:
    /// until it's removed as often.
    fn add_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self;

    /// Holds the lasting cue `tag` on this actor once more, this world's
    /// alone: a server doesn't replicate it.
    fn add_unreplicated_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self;

    /// Holds the lasting cue `tag` on this actor once less: a served source
    /// first, if both hold it.
    fn remove_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self;
}

impl CueCommands for EntityCommands<'_> {
    fn execute_cue(&mut self, cue: Cue) -> &mut Self {
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            execute(entity.into_world_mut(), actor, &cue, None, None);
        })
    }

    fn add_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self {
        let tag = tag.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            add(entity.into_world_mut(), actor, tag, false);
        })
    }

    fn add_unreplicated_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self {
        let tag = tag.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            add(entity.into_world_mut(), actor, tag, true);
        })
    }

    fn remove_cue(&mut self, tag: impl Into<TagKey>) -> &mut Self {
        let tag = tag.into();
        self.queue(move |entity: EntityWorldMut| {
            let actor = entity.id();
            remove_either(entity.into_world_mut(), actor, tag);
        })
    }
}

/// Playing cues on actors on the world at once, not as commands.
pub trait CueWorld {
    /// Plays the one-shot `cue` on `actor`.
    fn execute_cue(&mut self, actor: Entity, cue: &Cue);

    /// Holds the lasting cue `tag` on `actor` once more, served to clients.
    fn add_cue(&mut self, actor: Entity, tag: impl Into<TagKey>);

    /// Holds the lasting cue `tag` on `actor` once more, this world's alone.
    fn add_unreplicated_cue(&mut self, actor: Entity, tag: impl Into<TagKey>);

    /// Holds the lasting cue `tag` on `actor` once less: a served source
    /// first, if both hold it.
    fn remove_cue(&mut self, actor: Entity, tag: impl Into<TagKey>);
}

impl CueWorld for World {
    fn execute_cue(&mut self, actor: Entity, cue: &Cue) {
        execute(self, actor, cue, None, None);
    }

    fn add_cue(&mut self, actor: Entity, tag: impl Into<TagKey>) {
        add(self, actor, tag.into(), false);
    }

    fn add_unreplicated_cue(&mut self, actor: Entity, tag: impl Into<TagKey>) {
        add(self, actor, tag.into(), true);
    }

    fn remove_cue(&mut self, actor: Entity, tag: impl Into<TagKey>) {
        remove_either(self, actor, tag.into());
    }
}

/// One more source of the lasting `tag` on `actor`.
pub(crate) fn add(world: &mut World, actor: Entity, tag: TagKey, unreplicated: bool) {
    if let Ok(mut entity) = world.get_entity_mut(actor) {
        entity
            .entry::<CueSources>()
            .or_default()
            .get_mut()
            .add(tag, unreplicated);
    }
}

/// One less source of the lasting `tag` on `actor`.
pub(crate) fn remove(world: &mut World, actor: Entity, tag: TagKey, unreplicated: bool) {
    if let Some(mut sources) = world.get_mut::<CueSources>(actor) {
        sources.remove(tag, unreplicated);
    }
}

/// One less source of the lasting `tag` on `actor`, whichever holds it: a
/// served one first. Warned of if none does.
fn remove_either(world: &mut World, actor: Entity, tag: TagKey) {
    let removed = world
        .get_mut::<CueSources>(actor)
        .is_some_and(|mut sources| sources.remove(tag, false) || sources.remove(tag, true));
    if !removed {
        warn!("cue `{tag}` isn't held on {actor}: nothing removed");
    }
}

/// Plays the one-shot `cue` on `actor`, under `activation` and from `effect`
/// if so: triggered here; on a server, kept for its clients unless it's
/// unreplicated; on a client, under its own activation, noted so the
/// server's isn't played again.
pub(crate) fn execute(
    world: &mut World,
    actor: Entity,
    cue: &Cue,
    activation: Option<Activation>,
    effect: Option<EffectId>,
) {
    if world.get_entity(actor).is_err() {
        return;
    }
    let role = world.get_resource::<NetRole>().copied().unwrap_or_default();
    if !cue.unreplicated {
        match (role, activation) {
            (NetRole::Server, _) => burst(world, actor, cue, activation, effect),
            (NetRole::Client, Some(activation)) => {
                let at = world
                    .get_resource::<Time>()
                    .map_or(0.0, Time::elapsed_secs_f64);
                world
                    .get_resource_or_init::<Predicted>()
                    .note(actor, activation, cue.tag, at);
            }
            _ => {}
        }
    }
    world.trigger(CueExecuted {
        entity: actor,
        tag: cue.tag,
        changes: cue.changes.clone(),
        level: cue.level,
        context: cue.context.clone(),
        activation,
        effect,
    });
}

/// Keeps `cue` on `actor` for the server's clients, and lets go of those
/// older than a second.
fn burst(
    world: &mut World,
    actor: Entity,
    cue: &Cue,
    activation: Option<Activation>,
    effect: Option<EffectId>,
) {
    let Some(tags) = world.get_resource::<TagRegistry>() else {
        no_tag_tree();
        return;
    };
    let Some(tag) = tags.id(cue.tag) else {
        warn!("cue `{}` isn't in the tag tree: not replicated", cue.tag);
        return;
    };
    let data = world
        .get_resource::<ContextDataRegistry>()
        .cloned()
        .unwrap_or_default();
    let at = world
        .get_resource::<Time>()
        .map_or(0.0, Time::elapsed_secs_f64);
    let Ok(mut entity) = world.get_entity_mut(actor) else {
        return;
    };
    let mut bursts = entity.entry::<CueBursts>().or_default();
    let mut bursts = bursts.get_mut();
    bursts.recent.retain(|burst| at - burst.at <= KEPT);
    let seq = bursts.next;
    bursts.next += 1;
    bursts.recent.push(Burst {
        seq,
        at,
        tag,
        changes: cue.changes.clone(),
        level: cue.level,
        context: ContextReplica::of(&cue.context, &data),
        activation,
        effect,
    });
}

/// Actors whose lasting cues may have changed.
type Restated = Or<(Changed<CueSources>, Changed<ServedCues>)>;

/// An actor as its lasting cues are shown.
type Showing = (
    Entity,
    Option<&'static CueSources>,
    Option<&'static mut ServedCues>,
    Option<&'static ActiveCues>,
    SpawnDetails,
);

/// Shows each actor's lasting cues: those held here and those served, as
/// entities, spawned and despawned as they come and go; on a server, served to
/// its clients.
pub(crate) fn show(
    mut actors: Query<Showing, Restated>,
    mut cues: Query<&mut ActiveCue>,
    role: Res<NetRole>,
    tree: Option<Res<TagRegistry>>,
    mut commands: Commands,
) {
    let serving = *role == NetRole::Server;
    if serving && tree.is_none() && !actors.is_empty() {
        no_tag_tree();
    }
    let tree = tree.as_deref().cloned().unwrap_or_default();
    for (actor, sources, mut served, shown, spawned) in &mut actors {
        // A client just come to see the actor joins the cues it's served.
        let joining = spawned.is_spawned() && !serving;
        // Served at once, in place: replicated the same frame.
        if serving && let (Some(sources), Some(served)) = (sources, served.as_mut()) {
            let now: BTreeSet<TagId> = sources
                .served()
                .into_iter()
                .filter_map(|tag| {
                    let id = tree.id(tag);
                    if id.is_none() {
                        warn!("cue `{tag}` isn't in the tag tree: not served");
                    }
                    id
                })
                .collect();
            if served.0 != now {
                served.0 = now;
            }
        }
        let served: BTreeSet<TagKey> = served
            .as_deref()
            .into_iter()
            .flat_map(|served| &served.0)
            .filter_map(|&id| tree.key(id))
            .collect();
        let mut wanted: BTreeSet<TagKey> = sources.into_iter().flat_map(CueSources::iter).collect();
        wanted.extend(served.iter().copied());
        // Held only by what isn't replicated: neither served here nor to come.
        let unreplicated = |tag: TagKey| {
            !served.contains(&tag) && !sources.is_some_and(|sources| sources.serves(tag))
        };
        let shown: Vec<(Entity, TagKey)> = shown
            .into_iter()
            .flat_map(ActiveCues::iter)
            .filter_map(|cue| Some((cue, cues.get(cue).ok()?.tag)))
            .collect();
        for &(cue, tag) in &shown {
            if !wanted.contains(&tag) {
                commands.entity(cue).despawn();
                continue;
            }
            if let Ok(mut active) = cues.get_mut(cue) {
                let unreplicated = unreplicated(tag);
                if active.unreplicated != unreplicated {
                    active.unreplicated = unreplicated;
                }
            }
        }
        for tag in wanted {
            if shown.iter().any(|(_, shown)| *shown == tag) {
                continue;
            }
            commands.spawn((
                ActiveCue {
                    tag,
                    joined: joining && served.contains(&tag),
                    unreplicated: unreplicated(tag),
                },
                CueOf(actor),
            ));
        }
    }
}

/// On a client: plays the one-shots the server played on each actor that it
/// hasn't, but those it predicted itself. Coming to see an actor, it plays
/// what's in its buffer: the server keeps only what's just happened.
pub(crate) fn play(
    mut actors: Query<(Entity, &CueBursts, Option<&mut Played>), Changed<CueBursts>>,
    mut predicted: ResMut<Predicted>,
    tree: Option<Res<TagRegistry>>,
    data: Option<Res<ContextDataRegistry>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    let Some(tree) = tree.as_deref().cloned() else {
        if !actors.is_empty() {
            no_tag_tree();
        }
        return;
    };
    let data = data.as_deref().cloned().unwrap_or_default();
    predicted.forget(time.elapsed_secs_f64() - REMEMBERED);
    for (actor, bursts, played) in &mut actors {
        let from = if let Some(mut played) = played {
            std::mem::replace(&mut played.0, bursts.next)
        } else {
            commands.entity(actor).insert(Played(bursts.next));
            0
        };
        for burst in bursts.recent.iter().filter(|burst| burst.seq >= from) {
            let Some(tag) = tree.key(burst.tag) else {
                warn!("cue {:?} isn't in the tag tree: not played", burst.tag);
                continue;
            };
            if let Some(activation) = burst.activation
                && predicted.take(actor, activation, tag)
            {
                continue;
            }
            commands.trigger(CueExecuted {
                entity: actor,
                tag,
                changes: burst.changes.clone(),
                level: burst.level,
                context: burst.context.context(&data),
                activation: burst.activation,
                effect: burst.effect,
            });
        }
    }
}

/// On a server: lets go of the one-shots older than it keeps them for, so an
/// actor's buffer holds only what's just happened.
pub(crate) fn prune(mut actors: Query<&mut CueBursts>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for mut bursts in &mut actors {
        if bursts.recent.iter().any(|burst| now - burst.at > KEPT) {
            bursts.recent.retain(|burst| now - burst.at <= KEPT);
        }
    }
}

/// Warns, once, that there's no tag tree: cues go by its numbers, so without
/// it none is replicated, served or played.
fn no_tag_tree() {
    if crate::context::first_warning("no tag tree".to_owned()) {
        warn!(
            "there's no `TagRegistry`: cues go over the network by its numbers, so none is \
             replicated, served or played (loading content inserts it)"
        );
    }
}
