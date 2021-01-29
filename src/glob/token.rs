use itertools::Itertools as _;
use std::borrow::Cow;

use crate::glob::GlobError;

#[derive(Clone, Copy, Debug)]
pub enum Wildcard {
    One,        // ?
    ZeroOrMore, // *
    Tree,       // **
}

#[derive(Clone, Debug)]
pub enum Token<'t> {
    Literal(Cow<'t, str>),
    NonTreeSeparator,
    Wildcard(Wildcard),
}

impl<'t> Token<'t> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Literal(literal) => literal.into_owned().into(),
            Token::NonTreeSeparator => Token::NonTreeSeparator,
            Token::Wildcard(wildcard) => Token::Wildcard(wildcard),
        }
    }
}

impl<'t> From<&'t str> for Token<'t> {
    fn from(literal: &'t str) -> Self {
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

pub fn parse<'t>(text: &'t str) -> Result<Vec<Token<'t>>, GlobError> {
    use nom::bytes::complete as bytes;
    use nom::error::ParseError;
    use nom::{branch, combinator, multi, sequence, IResult, Parser};

    fn no_adjacent_tree<'i, O, E, F>(parser: F) -> impl FnMut(&'i str) -> IResult<&'i str, O, E>
    where
        E: ParseError<&'i str>,
        F: Parser<&'i str, O, E>,
    {
        sequence::delimited(
            combinator::peek(combinator::not(bytes::tag("**"))),
            parser,
            combinator::peek(combinator::not(bytes::tag("**"))),
        )
    }

    // TODO: Support escaping wildcards as literals.
    fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        combinator::map(no_adjacent_tree(bytes::is_not("/?*")), From::from)(input)
    }

    fn wildcard<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        branch::alt((
            combinator::map(no_adjacent_tree(bytes::tag("?")), |_| {
                Token::from(Wildcard::One)
            }),
            combinator::map(
                sequence::delimited(
                    branch::alt((bytes::tag("/"), bytes::tag(""))),
                    bytes::tag("**"),
                    branch::alt((bytes::tag("/"), combinator::eof)),
                ),
                |_| Token::from(Wildcard::Tree),
            ),
            combinator::map(
                sequence::terminated(
                    bytes::tag("*"),
                    branch::alt((combinator::peek(bytes::is_not("*")), combinator::eof)),
                ),
                |_| Token::from(Wildcard::ZeroOrMore),
            ),
        ))(input)
    }

    fn separator<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        combinator::value(Token::NonTreeSeparator, bytes::tag("/"))(input)
    }

    combinator::all_consuming(multi::many1(branch::alt((wildcard, separator, literal))))(text)
        .map(|(_, tokens)| tokens)
        .map_err(From::from)
}

pub fn coalesce<'t>(
    tokens: impl IntoIterator<Item = Token<'t>>,
) -> impl Iterator<Item = Token<'t>> {
    tokens
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
        .dedup_by(|left, right| {
            matches!(
                (left, right),
                (
                    Token::Wildcard(Wildcard::ZeroOrMore),
                    Token::Wildcard(Wildcard::ZeroOrMore)
                )
            )
        })
        .filter(|token| match &token {
            Token::Literal(ref literal) => !literal.is_empty(),
            _ => true,
        })
        .coalesce(|left, right| match (&left, &right) {
            (Token::Literal(ref left), Token::Literal(ref right)) => {
                Ok(Token::Literal(format!("{}{}", left, right).into()))
            }
            _ => Err((left, right)),
        })
}
