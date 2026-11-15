//! What can be wrong with content, or a name looked up: `Error`, and every
//! error found at once, `Errors`.

use std::fmt;

use crate::registry::Kind;

/// What can be wrong with content: a tag, an effect, a modifier.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Error {
    /// Two definitions of one kind share a name.
    #[error("{kind} `{name}` is defined twice")]
    Duplicate {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The name they share.
        name: String,
    },
    /// A name that isn't defined.
    #[error("there's no {kind} called `{name}`{}", context(.within.as_deref()))]
    Unknown {
        /// What kind of thing: `tag`, `item`.
        kind: &'static str,
        /// The name.
        name: String,
        /// Where it was named, if in a definition: its name.
        within: Option<String>,
    },
    /// Content defining what a type in code defines: content adds, it
    /// doesn't redefine. A curve is the exception, as data tunes curves.
    #[error("content defines {kind} `{name}`, which code defines")]
    DefinedInCode {
        /// What kind of thing: `effect`, `ability`.
        kind: &'static str,
        /// The key they share.
        name: String,
    },
    /// Something that can't be: a number that isn't finite, a `per` of 0 or
    /// less, a ramp from a value to itself.
    #[error("{what}{}", context(.within.as_deref()))]
    Invalid {
        /// What's wrong, in words.
        what: String,
        /// Where, if in a definition: its name.
        within: Option<String>,
    },
}

impl Error {
    /// That `what` can't be, named within `within` if in a definition.
    #[must_use]
    pub fn invalid(what: impl Into<String>, within: Option<&str>) -> Self {
        Self::Invalid {
            what: what.into(),
            within: within.map(ToOwned::to_owned),
        }
    }

    /// That there's no `K` called `name`, named within `within` if in a
    /// definition.
    #[must_use]
    pub fn unknown<K: Kind>(name: &str, within: Option<&str>) -> Self {
        Self::Unknown {
            kind: K::NAME,
            name: name.to_owned(),
            within: within.map(ToOwned::to_owned),
        }
    }
}

fn context(within: Option<&str>) -> String {
    within
        .map(|stat| format!(" (in `{stat}`)"))
        .unwrap_or_default()
}

/// Everything wrong with content, found at once: each [`Error`] checking found.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub struct Errors(pub Vec<Error>);

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.0.iter().enumerate() {
            if index > 0 {
                f.write_str("; ")?;
            }
            write!(f, "{error}")?;
        }
        Ok(())
    }
}
