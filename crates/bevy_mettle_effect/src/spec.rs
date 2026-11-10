//! What's applied: an effect, at a level, with what code set, from whom.

use bevy::prelude::*;
use bevy_mettle_core::Activation;
use bevy_mettle_core::registry::{AnyEffect, AttributeId, EffectId, Keyed, Ref};
use bevy_mettle_core::tag::{TagKey, Tags};

use crate::EffectRegistry;
use crate::context::{ContextData, EffectContext};
use crate::data::{CaptureDef, Read, Side};

/// An effect code names: a marker type, its content's name on it.
///
/// Derive it, add it to the app, and apply it: the type is its spec, so
/// code names effects without a string, and a misspelled one doesn't compile.
///
/// ```ignore
/// #[derive(Effect)]
/// #[effect(name = "burning")]
/// pub struct Burning;
///
/// app.add_effect::<Burning>();
/// commands.entity(mage).apply_effect(Burning.level(3.0).param(DamageParam, -5.0));
/// ```
///
/// [`add_effect`](crate::code::EffectApp::add_effect) makes content
/// without it fail its check, so its name is checked once, as content loads.
/// Its builders make an [`EffectSpec`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` isn't an effect",
    label = "not an effect",
    note = "derive it: `#[derive(Effect)]`"
)]
pub trait Effect: Keyed<Kind = AnyEffect> + Send + Sync + 'static {
    /// It, at level 1, from no one.
    #[must_use]
    fn spec(self) -> EffectSpec
    where
        Self: Sized,
    {
        EffectSpec::of::<Self>()
    }

    /// It, at `level`.
    #[must_use]
    fn level(self, level: f32) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().level(level)
    }

    /// It, with `param` (a tag: `param.damage`) at `value`.
    #[must_use]
    fn param(self, param: impl Into<TagKey>, value: f32) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().param(param, value)
    }

    /// It, from `source`.
    #[must_use]
    fn source(self, source: Entity) -> EffectSpec
    where
        Self: Sized,
    {
        self.spec().source(source)
    }
}

pub use bevy_mettle_macros::Effect;

impl<E: Effect> From<E> for EffectSpec {
    fn from(effect: E) -> Self {
        effect.spec()
    }
}

/// An effect about to be applied: which, at what level, with what, from whom.
///
/// Unreal's `GameplayEffectSpec`: with what the applying code set. Made from
/// an [`Effect`] type (`Burning.level(3.0)`); its effect is resolved to its
/// number as it's applied. An active effect keeps the spec it was applied
/// with.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectSpec {
    effect: Ref<AnyEffect>,
    /// Its level: what curves are read at. 1 unless it says.
    pub level: f32,
    /// What the applying code set, for `Param` magnitudes.
    pub params: Vec<(TagKey, f32)>,
    /// What it carries beyond them: who applied it, and context data.
    pub context: EffectContext,
    /// Its source's attributes as they were when it was made, for its
    /// effect's snapshot captures: taken by
    /// [`snapshot`](crate::EffectWorld::snapshot), or as it's applied.
    pub snapshot: Vec<Snapshot>,
    /// The activation it was applied under, if any: what a rejected prediction
    /// takes back. The server's own are apart from a client's; a client's
    /// is told apart from another client's by the source.
    pub activation: Option<Activation>,
}

impl EffectSpec {
    /// `E`, at level 1, from no one.
    #[must_use]
    pub fn of<E: Effect>() -> Self {
        Self::new(Ref::of::<E>())
    }

    /// The effect numbered `effect`, at level 1, from no one: for mettle's
    /// crates, which know effects by number (an ability's cost, a replica).
    #[doc(hidden)]
    #[must_use]
    pub fn from_id(effect: EffectId) -> Self {
        Self::new(Ref::Id(effect))
    }

    fn new(effect: Ref<AnyEffect>) -> Self {
        Self {
            effect,
            level: 1.0,
            params: Vec::new(),
            context: EffectContext::default(),
            snapshot: Vec::new(),
            activation: None,
        }
    }

    /// At `level`.
    #[must_use]
    pub fn level(mut self, level: f32) -> Self {
        self.level = level;
        self
    }

    /// With `param` (a tag: `param.damage`) at `value`. Setting one again
    /// replaces it.
    #[must_use]
    pub fn param(mut self, param: impl Into<TagKey>, value: f32) -> Self {
        let param = param.into();
        match self.params.iter_mut().find(|(known, _)| *known == param) {
            Some((_, known)) => *known = value,
            None => self.params.push((param, value)),
        }
        self
    }

    /// From `source`: who's responsible, the caster.
    #[must_use]
    pub fn source(mut self, source: Entity) -> Self {
        self.context = self.context.source(source);
        self
    }

    /// With `data` on its context, in place of any of its type: mettle's own
    /// ([`Causer`](crate::context::Causer), [`Hit`](crate::context::Hit), ..)
    /// or the game's.
    #[must_use]
    pub fn with<C: ContextData>(mut self, data: C) -> Self {
        self.context = self.context.with(data);
        self
    }

    /// Under `activation`.
    #[must_use]
    pub fn activation(mut self, activation: Activation) -> Self {
        self.activation = Some(activation);
        self
    }

    /// What `param` was set to, if it was.
    #[must_use]
    pub fn param_value(&self, param: impl Into<TagKey>) -> Option<f32> {
        let param = param.into();
        self.params
            .iter()
            .find(|(known, _)| *known == param)
            .map(|(_, value)| *value)
    }

    /// What its snapshot holds for `capture`, if it's of its source and the
    /// spec took it.
    #[must_use]
    pub fn snapshot_of(&self, capture: &CaptureDef) -> Option<f32> {
        if capture.from != Side::Source {
            return None;
        }
        self.snapshot
            .iter()
            .find(|taken| {
                taken.attribute == capture.attribute
                    && taken.value == capture.value
                    && taken.tags == capture.tags
            })
            .map(|taken| taken.amount)
    }

    /// Its effect's number in `registry`: `None` if there's no such effect, or
    /// its type isn't added.
    #[must_use]
    pub fn effect_id(&self, registry: &EffectRegistry) -> Option<EffectId> {
        registry.find(self.effect)
    }

    /// The same, for `effect`: what an effect it chains is applied with.
    pub(crate) fn for_effect(&self, effect: EffectId) -> Self {
        Self {
            effect: Ref::Id(effect),
            ..self.clone()
        }
    }

    /// What its effect is called, for a warning: its type's name, or its
    /// number.
    pub(crate) fn label(&self) -> String {
        self.effect.label()
    }
}

/// A source's attribute as a spec was made, for a snapshot capture of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The attribute.
    pub attribute: AttributeId,
    /// Which of its values.
    pub value: Read,
    /// What it was read in.
    pub tags: Tags,
    /// What it was.
    pub amount: f32,
}
