#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
// The README is the crate's documentation. Its example needs the default
// parts, so it's doctested only with them, through `ReadmeDoctests`.
#![cfg_attr(not(doctest), doc = include_str!("../README.md"))]

use bevy::app::plugin_group;

mod content;
mod order;

/// Abilities: granted, activated, paid for, cooled down and ended.
#[cfg(feature = "ability")]
#[cfg_attr(docsrs, doc(cfg(feature = "ability")))]
#[doc(inline)]
pub use bevy_mettle_ability as ability;
/// Attributes: bases, current values, modifiers, reads in context.
#[cfg(feature = "attribute")]
#[cfg_attr(docsrs, doc(cfg(feature = "attribute")))]
#[doc(inline)]
pub use bevy_mettle_attribute as attribute;
/// What every part shares: tags, curves, conditions and registries.
#[doc(inline)]
pub use bevy_mettle_core as core;
/// Effects: instant, lasting and periodic, stacking and granting tags.
#[cfg(feature = "effect")]
#[cfg_attr(docsrs, doc(cfg(feature = "effect")))]
#[doc(inline)]
pub use bevy_mettle_effect as effect;
/// Prediction over lightyear: replicas, requests and replies sent.
#[cfg(feature = "lightyear")]
#[cfg_attr(docsrs, doc(cfg(feature = "lightyear")))]
#[doc(inline)]
pub use bevy_mettle_lightyear as lightyear;

pub use self::content::{ContentLoaded, ContentRejected, MettleContent, MettleContentPlugin};
pub use self::order::{MettleSystems, MettleSystemsPlugin};

pub mod prelude {
    //! What using the enabled parts takes: the preludes of every crate enabled.
    //!
    //! `use bevy_mettle::prelude::*`.

    #[cfg(feature = "ability")]
    #[doc(no_inline)]
    pub use bevy_mettle_ability::prelude::*;
    #[cfg(feature = "attribute")]
    #[doc(no_inline)]
    pub use bevy_mettle_attribute::prelude::*;
    #[doc(no_inline)]
    pub use bevy_mettle_core::prelude::*;
    #[cfg(feature = "effect")]
    #[doc(no_inline)]
    pub use bevy_mettle_effect::prelude::*;
    #[cfg(feature = "lightyear")]
    #[doc(no_inline)]
    pub use bevy_mettle_lightyear::prelude::*;

    #[doc(no_inline)]
    pub use crate::{ContentLoaded, ContentRejected, MettleContent, MettlePlugins, MettleSystems};
}

plugin_group! {
    /// The plugin of every part its features enable, each with its defaults.
    ///
    /// Add it before any `add_attribute`, which adds `AttributePlugin` itself
    /// if it isn't there yet. With `lightyear`, add it after lightyear's own
    /// plugins, with mettle's `NetRole` set. Each keeps its defaults; change
    /// one with `MettlePlugins.set(EffectPlugin::new(Update))`.
    pub struct MettlePlugins {
        #[cfg(feature = "attribute")]
        attribute:::AttributePlugin,
        #[cfg(feature = "effect")]
        effect:::EffectPlugin,
        #[cfg(feature = "ability")]
        ability:::AbilityPlugin,
        :MettleSystemsPlugin,
        :MettleContentPlugin,
        #[cfg(feature = "lightyear")]
        lightyear:::MettleNetPlugin,
    }
}

/// The README's example, compiled as a doctest with the default parts: a
/// game without lightyear.
#[cfg(all(doctest, feature = "ability", not(feature = "lightyear")))]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

/// The book's examples, one item per chapter, compiled as doctests. The book
/// isn't published with the crate, so this only builds in the repository.
///
/// A game without lightyear has `MettlePlugins` without `MettleNetPlugin`,
/// so the chapters are compiled with the default parts and not `lightyear`;
/// the lightyear chapter, with it.
#[cfg(doctest)]
mod book {
    macro_rules! chapters {
        ($($(#[$cfg:meta])* $name:ident: $file:literal),* $(,)?) => {
            $(
                $(#[$cfg])*
                #[doc = include_str!(concat!("../../../book/src/", $file))]
                struct $name;
            )*
        };
    }

    chapters! {
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Introduction: "introduction.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Installation: "installation.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        FirstAttributes: "first-attributes.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Concepts: "concepts.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Tags: "tags.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Curves: "curves.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Conditions: "conditions.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Attributes: "attributes.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Modifiers: "modifiers.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Reading: "reading.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Effects: "effects.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        ApplyingEffects: "applying-effects.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Stacking: "stacking.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        EffectCode: "effect-code.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Abilities: "abilities.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Activating: "activating.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Input: "input.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        GrantsTriggers: "grants-triggers.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Cues: "cues.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Prediction: "prediction.md",
        #[cfg(feature = "lightyear")]
        Lightyear: "lightyear.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        LoadingData: "loading-data.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Testing: "testing.md",
        #[cfg(all(feature = "ability", not(feature = "lightyear")))]
        Features: "features.md",
    }
}
