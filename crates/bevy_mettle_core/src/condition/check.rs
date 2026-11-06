//! Checking an expression against the tag tree.

use super::Condition;
use crate::MettleError;
use crate::tag::TagTree;

impl Condition {
    /// Checks every tag in it is defined in `tags`, and its numbers can be:
    /// named within `within` (what holds it) if it's in something.
    ///
    /// # Errors
    ///
    /// The first tag that isn't defined, or number that can't be.
    pub fn check(&self, tags: &TagTree, within: Option<&str>) -> Result<(), MettleError> {
        let invalid = |what: String| MettleError::Invalid {
            what,
            within: within.map(ToOwned::to_owned),
        };
        match self {
            Self::All(terms) | Self::Any(terms) => {
                terms.iter().try_for_each(|term| term.check(tags, within))
            }
            Self::Not(term) => term.check(tags, within),
            Self::ReadTagged(tag)
            | Self::OwnerHas(tag)
            | Self::TargetHas(tag)
            | Self::SourceHas(tag) => tags.known(tag, within),
            Self::FactSet(fact) => tags.known(fact, within),
            Self::FactAbove { fact, value } | Self::FactBelow { fact, value } => {
                tags.known(fact, within)?;
                if value.is_finite() {
                    Ok(())
                } else {
                    Err(invalid(format!("a comparison of `{fact}` with {value}")))
                }
            }
            Self::FactRamp { fact, full, none } => {
                tags.known(fact, within)?;
                if full.is_finite() && none.is_finite() && (none - full).abs() > 0.0 {
                    Ok(())
                } else {
                    Err(invalid(format!(
                        "a ramp of `{fact}` from {full} to {none}: they have to differ"
                    )))
                }
            }
            Self::FactEvery { fact, n } => {
                tags.known(fact, within)?;
                if *n > 0 {
                    Ok(())
                } else {
                    Err(invalid(format!("every 0 of `{fact}`")))
                }
            }
        }
    }
}
