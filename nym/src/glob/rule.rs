use thiserror::Error;

use crate::glob::token::{self, Component, Token};
use crate::glob::{IteratorExt as _, SliceExt as _, Terminals};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RuleError {
    #[error("invalid tree wildcard `**` in alternative")]
    AlternativeTree,
    #[error("invalid zero-or-more wildcard `*` or `$` in alternative")]
    AlternativeZeroOrMore,
}

pub fn check<'t, I>(tokens: I) -> Result<(), RuleError>
where
    I: IntoIterator<Item = &'t Token<'t>>,
    I::IntoIter: Clone,
{
    let tokens = tokens.into_iter();
    alternative(tokens)?;
    Ok(())
}

fn alternative<'t, I>(tokens: I) -> Result<(), RuleError>
where
    I: IntoIterator<Item = &'t Token<'t>>,
    I::IntoIter: Clone,
{
    use crate::glob::token::Token::{Alternative, Wildcard};
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

    fn check<'t>(
        terminals: Terminals<&Token<'t>>,
        left: Option<&Token<'t>>,
        right: Option<&Token<'t>>,
    ) -> Result<(), RuleError> {
        match terminals {
            // TODO: Do not consider this an error and instead detect this in
            //       `token::optimize` and replace `{...,**,...}` with `**`.
            Only(Wildcard(Tree)) => {
                // Disallow singular tree tokens.
                //
                // For example, `{foo,**}`.
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
