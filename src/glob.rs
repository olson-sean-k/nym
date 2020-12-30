use nom::error::ErrorKind;
use std::borrow::Cow;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GlobError {
    #[error("failed to parse glob")]
    Parse,
}

impl<I> From<nom::Err<(I, ErrorKind)>> for GlobError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        GlobError::Parse
    }
}

#[derive(Clone, Copy, Debug)]
enum Wildcard {
    One,  // ?
    Many, // *
    Tree, // **
}

#[derive(Clone, Debug)]
enum Token<'a> {
    Literal(Cow<'a, str>),
    Wildcard(Wildcard),
}

impl<'a> Token<'a> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Literal(literal) => literal.into_owned().into(),
            Token::Wildcard(wildcard) => Token::Wildcard(wildcard),
        }
    }
}

impl<'a> From<&'a str> for Token<'a> {
    fn from(literal: &'a str) -> Self {
        Token::Literal(literal.into())
    }
}

impl From<String> for Token<'static> {
    fn from(literal: String) -> Self {
        Token::Literal(literal.into())
    }
}

impl From<Wildcard> for Token<'static> {
    fn from(wildcard: Wildcard) -> Self {
        Token::Wildcard(wildcard)
    }
}

#[derive(Clone, Debug)]
pub struct Glob<'a> {
    tokens: Vec<Token<'a>>,
}

impl<'a> Glob<'a> {
    pub fn parse(text: &'a str) -> Result<Self, GlobError> {
        use nom::bytes::complete as bytes;
        use nom::error::ParseError;
        use nom::{branch, combinator, multi, sequence, IResult};

        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(bytes::is_not("?*"), From::from)(input)
        }

        fn wildcard<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            branch::alt((
                combinator::map(bytes::tag("?"), |_| Token::from(Wildcard::One)),
                // TODO: Only allow path separators as delimiters of tree wildcards.
                sequence::terminated(
                    combinator::map(bytes::tag("**"), |_| Token::from(Wildcard::Tree)),
                    branch::alt((bytes::is_not("*"), combinator::eof)),
                ),
                sequence::terminated(
                    combinator::map(bytes::tag("*"), |_| Token::from(Wildcard::Many)),
                    branch::alt((bytes::is_not("*"), combinator::eof)),
                ),
            ))(input)
        }

        fn glob<'i, E>(input: &'i str) -> IResult<&'i str, Glob, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map(
                multi::many1(branch::alt((literal, wildcard))),
                move |tokens| Glob { tokens },
            ))(input)
        }

        glob::<(_, ErrorKind)>(text)
            .map(|(_, glob)| glob)
            .map_err(Into::into)
    }

    pub fn into_owned(self) -> Glob<'static> {
        let Glob { tokens } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens }
    }
}

impl FromStr for Glob<'static> {
    type Err = GlobError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Glob::parse(text).map(|glob| glob.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::glob::Glob;

    #[test]
    fn parse_any_tokens() {
        Glob::parse("*").unwrap();
        Glob::parse("a/*").unwrap();
        Glob::parse("*a").unwrap();
        Glob::parse("a*").unwrap();
        Glob::parse("a*b").unwrap();
    }

    #[test]
    fn parse_one_tokens() {
        Glob::parse("?").unwrap();
        Glob::parse("a/?").unwrap();
        Glob::parse("?a").unwrap();
        Glob::parse("a?").unwrap();
        Glob::parse("a?b").unwrap();
        Glob::parse("??a??b??").unwrap();
    }

    #[test]
    fn parse_tree_tokens() {
        Glob::parse("**").unwrap();
        Glob::parse("**/").unwrap();
        Glob::parse("/**").unwrap();
        Glob::parse("**/a").unwrap();
        Glob::parse("a/**").unwrap();
    }

    #[test]
    fn reject_adjacent_any_or_tree_tokens() {
        assert!(Glob::parse("***").is_err());
        assert!(Glob::parse("****").is_err());
        assert!(Glob::parse("**/*/***").is_err());
    }
}
