//! Rules and limitations for token sequences.
//!
//! This module provides the `check` function, which examines a token sequence
//! and emits an error if the sequence violates rules. Rules are invariants that
//! are difficult or impossible to enforce when parsing text and primarily
//! detect and reject token sequences that produce anomalous, meaningless, or
//! unexpected globs (regular expressions) when compiled.
//!
//! Most rules concern alternatives, which have complex interactions with
//! neighboring tokens.

use itertools::Itertools as _;
use thiserror::Error;

use crate::glob::token::{self, Component, Token};
use crate::glob::{IteratorExt as _, SliceExt as _, Terminals};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RuleError {
    #[error("invalid separator `/` in alternative")]
    AlternativeSeparator,
    #[error("invalid tree wildcard `**` in alternative")]
    AlternativeTree,
    #[error("invalid zero-or-more wildcard `*` or `$` in alternative")]
    AlternativeZeroOrMore,
    #[error("adjacent component boundaries `/` or `**`")]
    BoundaryAdjacent,
}

pub fn check<'t, I>(tokens: I) -> Result<(), RuleError>
where
    I: IntoIterator<Item = &'t Token<'t>>,
    I::IntoIter: Clone,
{
    let tokens = tokens.into_iter();
    alternative(tokens.clone())?;
    boundary(tokens)?;
    Ok(())
}

fn alternative<'t, I>(tokens: I) -> Result<(), RuleError>
where
    I: IntoIterator<Item = &'t Token<'t>>,
    I::IntoIter: Clone,
{
    use crate::glob::token::Token::{Alternative, Separator, Wildcard};
    use crate::glob::token::Wildcard::{Tree, ZeroOrMore};
    use crate::glob::Terminals::{Only, StartEnd};

    fn recurse<'t>(
        components: impl Iterator<Item = Component<'t>>,
        parent: (Option<&Token<'t>>, Option<&Token<'t>>),
    ) -> Result<(), RuleError> {
        for component in components {
            for (left, alternative, right) in
                component
                    .tokens()
                    .iter()
                    .adjacent()
                    .filter_map(|adjacency| match adjacency.into_tuple() {
                        (left, Alternative(alternative), right) => Some((left, alternative, right)),
                        _ => None,
                    })
            {
                let left = left.cloned().or(parent.0);
                let right = right.cloned().or(parent.1);
                for tokens in alternative.branches() {
                    if let Some(terminals) = tokens.terminals() {
                        // Check branch terminals against the tokens adjacent to
                        // their corresponding alternative token.
                        check(terminals, left, right)?;
                    }
                    recurse(token::components(tokens), (left, right))?;
                }
            }
        }
        Ok(())
    }

    // NOTE: Terminal tree tokens are permitted even when an alternative is
    //       adjacent to components or terminations (separators). Such tree
    //       tokens compose with separators, because they compile as prefix or
    //       postfix forms despite being intermediate to the glob. This differs
    //       from terminal separators within an alternative, which do not
    //       compose and are rejected when adjacent to components or
    //       terminations. For example, `{foo/**}/bar` is allowed (note the
    //       separator in `/bar`) but `{foo/}/bar` is not.
    fn check<'t>(
        terminals: Terminals<&Token<'t>>,
        left: Option<&Token<'t>>,
        right: Option<&Token<'t>>,
    ) -> Result<(), RuleError> {
        match terminals {
            Only(Separator) if left.is_none() || right.is_none() => {
                // The alternative is adjacent to components or terminations;
                // disallow singular separators.
                //
                // For example, `foo/{bar,/}`.
                Err(RuleError::AlternativeSeparator)
            }
            StartEnd(Separator, _) if left.is_none() => {
                // The alternative is preceded by components or terminations;
                // disallow leading separators.
                //
                // For example, `foo/{bar,/baz}`.
                Err(RuleError::AlternativeSeparator)
            }
            StartEnd(_, Separator) if right.is_none() => {
                // The alternative is followed by components or terminations;
                // disallow trailing separators.
                //
                // For example, `{foo,bar/}/baz`.
                Err(RuleError::AlternativeSeparator)
            }
            Only(Wildcard(Tree)) => {
                // NOTE: Supporting singular tree tokens is possible, but
                //       presents subtle edge cases that may be misleading or
                //       confusing. Rather than optimize or otherwise allow
                //       singular tree tokens, they are forbidden for
                //       simplicity.
                // Disallow singular tree tokens.
                //
                // For example, `{foo,bar,**}`.
                Err(RuleError::AlternativeTree)
            }
            StartEnd(Wildcard(Tree), _) if left.is_some() => {
                // The alternative is prefixed; disallow leading tree tokens.
                //
                // For example, `foo{bar,**/baz}`.
                Err(RuleError::AlternativeTree)
            }
            StartEnd(_, Wildcard(Tree)) if right.is_some() => {
                // The alternative is postfixed; disallow trailing tree tokens.
                //
                // For example, `{foo,bar/**}baz`.
                Err(RuleError::AlternativeTree)
            }
            Only(Wildcard(ZeroOrMore(_)))
                if matches!(
                    (left, right),
                    (Some(Wildcard(ZeroOrMore(_))), _) | (_, Some(Wildcard(ZeroOrMore(_))))
                ) =>
            {
                // The alternative is adjacent to a zero-or-more token; disallow
                // singular zero-or-more tokens.
                //
                // For example, `foo*{bar,*,baz}`.
                Err(RuleError::AlternativeZeroOrMore)
            }
            StartEnd(Wildcard(ZeroOrMore(_)), _)
                if matches!(left, Some(Wildcard(ZeroOrMore(_)))) =>
            {
                // The alternative is prefixed by a zero-or-more token; disallow
                // leading zero-or-more tokens.
                //
                // For example, `foo*{bar,*baz}`.
                Err(RuleError::AlternativeZeroOrMore)
            }
            StartEnd(_, Wildcard(ZeroOrMore(_)))
                if matches!(right, Some(Wildcard(ZeroOrMore(_)))) =>
            {
                // The alternative is postfixed by a zero-or-more token;
                // disallow trailing zero-or-more tokens.
                //
                // For example, `{foo,bar*}*baz`.
                Err(RuleError::AlternativeZeroOrMore)
            }
            _ => Ok(()),
        }
    }

    recurse(token::components(tokens), (None, None))
}

fn boundary<'t, I>(tokens: I) -> Result<(), RuleError>
where
    I: IntoIterator<Item = &'t Token<'t>>,
    I::IntoIter: Clone,
{
    if tokens
        .into_iter()
        .tuple_windows::<(_, _)>()
        .any(|(left, right)| left.is_component_boundary() && right.is_component_boundary())
    {
        Err(RuleError::BoundaryAdjacent)
    }
    else {
        Ok(())
    }
}
