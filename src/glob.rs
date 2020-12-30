use bstr::ByteVec;
use itertools::Itertools as _;
use nom::error::ErrorKind;
use regex::bytes::Regex;
use std::borrow::Cow;
use std::path::Path;
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

#[derive(Clone, Debug)]
pub struct BytePath<'a> {
    path: Cow<'a, [u8]>,
}

impl<'a> BytePath<'a> {
    pub fn new<P>(path: &'a P) -> Self
    where
        P: AsRef<Path>,
    {
        #[cfg(unix)]
        fn normalize(mut path: Cow<[u8]>) -> Cow<[u8]> {
            path
        }

        #[cfg(not(unix))]
        fn normalize(mut path: Cow<[u8]>) -> Cow<[u8]> {
            use std::path;

            for i in 0..path.len() {
                if path[i] == b'/' || !path::is_separator(path[i] as char) {
                    continue;
                }
                path.to_mut()[i] = b'/';
            }
            path
        }

        let path = normalize(Vec::from_path_lossy(path.as_ref()));
        BytePath { path }
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
    regex: Regex,
}

impl<'a> Glob<'a> {
    fn from_tokens<I>(tokens: I) -> Result<Self, GlobError>
    where
        I: IntoIterator<Item = Token<'a>>,
    {
        const ASCII_TERMINATOR: u8 = 0x7F;
        let tokens: Vec<_> = tokens
            .into_iter()
            .dedup_by(|left, right| {
                matches!(
                    (left, right),
                    (
                        Token::Wildcard(Wildcard::Tree),
                        Token::Wildcard(Wildcard::Tree)
                    )
                )
            })
            .collect();
        let mut pattern = String::new();
        pattern.push_str("(?-u)^");
        for token in tokens.iter() {
            match token {
                Token::Literal(ref literal) => {
                    let bytes = literal.as_bytes();
                    for &byte in bytes {
                        if byte <= ASCII_TERMINATOR {
                            pattern.push_str(&regex::escape(&(byte as char).to_string()));
                        }
                        else {
                            pattern.push_str(&format!("\\x{:02x}", byte));
                        }
                    }
                }
                Token::Wildcard(Wildcard::One) => pattern.push_str("([^/])"),
                Token::Wildcard(Wildcard::Many) => pattern.push_str("([^/]*)"),
                Token::Wildcard(Wildcard::Tree) => pattern.push_str("(/|/.*/)"),
            }
        }
        pattern.push_str("$");
        let regex = Regex::new(&pattern).map_err(|_| GlobError::Parse)?;
        Ok(Glob { tokens, regex })
    }

    pub fn parse(text: &'a str) -> Result<Self, GlobError> {
        use nom::bytes::complete as bytes;
        use nom::error::{FromExternalError, ParseError};
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
            E: FromExternalError<&'i str, GlobError> + ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map_res(
                multi::many1(branch::alt((literal, wildcard))),
                move |tokens| Glob::from_tokens(tokens),
            ))(input)
        }

        glob::<(_, ErrorKind)>(text)
            .map(|(_, glob)| glob)
            .map_err(Into::into)
    }

    pub fn into_owned(self) -> Glob<'static> {
        // Taking ownership of token data does not modify the regular
        // expression.
        let Glob { tokens, regex } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens, regex }
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
