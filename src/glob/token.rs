use itertools::Itertools as _;
use std::borrow::Cow;

use crate::glob::GlobError;

#[derive(Clone, Copy, Debug)]
pub enum Evaluation {
    Eager,
    Lazy,
}

#[derive(Clone, Copy, Debug)]
pub enum Archetype {
    Character(char),
    Range(char, char),
}

impl From<char> for Archetype {
    fn from(literal: char) -> Archetype {
        Archetype::Character(literal)
    }
}

impl From<(char, char)> for Archetype {
    fn from(range: (char, char)) -> Archetype {
        Archetype::Range(range.0, range.1)
    }
}

#[derive(Clone, Debug)]
pub enum Wildcard {
    One,
    Class {
        is_negated: bool,
        archetypes: Vec<Archetype>,
    },
    ZeroOrMore(Evaluation),
    Tree,
}

#[derive(Clone, Debug)]
pub enum Token<'t> {
    Alternative(Vec<Vec<Token<'t>>>),
    Literal(Cow<'t, str>),
    NonTreeSeparator,
    Wildcard(Wildcard),
}

impl<'t> Token<'t> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Alternative(alternatives) => Token::Alternative(
                alternatives
                    .into_iter()
                    .map(|tokens| tokens.into_iter().map(|token| token.into_owned()).collect())
                    .collect(),
            ),
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

// TODO: Patterns like `/**` do not parse correctly. The initial separator is
//       considered a part of a tree token. This means that the root is lost,
//       such that `/**` and `**` are equivalent.
//
//       This should be fixed, but note that solutions that introduce invalid
//       token sequences should be avoided! If possible, arbitrary token
//       sequences should always be valid.
pub fn parse(text: &str) -> Result<Vec<Token<'_>>, GlobError> {
    use nom::bytes::complete as bytes;
    use nom::character::complete as character;
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

    fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        combinator::map(
            combinator::verify(
                // NOTE: Character classes, which accept arbitrary characters,
                //       can be used to escape metacharacters like `*`, `?`,
                //       etc. For example, to escape `*`, either `\*` or `[*]`
                //       can be used.
                bytes::escaped_transform(
                    no_adjacent_tree(bytes::is_not("/?*$[]{},\\")),
                    '\\',
                    branch::alt((
                        combinator::value("?", bytes::tag("?")),
                        combinator::value("*", bytes::tag("*")),
                        combinator::value("$", bytes::tag("$")),
                        combinator::value("[", bytes::tag("[")),
                        combinator::value("]", bytes::tag("]")),
                        combinator::value("{", bytes::tag("{")),
                        combinator::value("}", bytes::tag("}")),
                        combinator::value(",", bytes::tag(",")),
                        combinator::value("\\", bytes::tag("\\")),
                    )),
                ),
                |text: &str| !text.is_empty(),
            ),
            Token::from,
        )(input)
    }

    fn wildcard<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        fn archetypes<'i, E>(input: &'i str) -> IResult<&'i str, Vec<Archetype>, E>
        where
            E: ParseError<&'i str>,
        {
            let escaped_character = |input| {
                branch::alt((
                    character::none_of("[]-\\"),
                    branch::alt((
                        combinator::value('[', bytes::tag("\\[")),
                        combinator::value(']', bytes::tag("\\]")),
                        combinator::value('-', bytes::tag("\\-")),
                    )),
                ))(input)
            };

            multi::many1(branch::alt((
                combinator::map(
                    sequence::separated_pair(escaped_character, bytes::tag("-"), escaped_character),
                    Archetype::from,
                ),
                combinator::map(escaped_character, Archetype::from),
            )))(input)
        }

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
                |_| Wildcard::Tree.into(),
            ),
            combinator::map(
                sequence::terminated(
                    bytes::tag("*"),
                    branch::alt((combinator::peek(bytes::is_not("*$")), combinator::eof)),
                ),
                |_| Wildcard::ZeroOrMore(Evaluation::Eager).into(),
            ),
            combinator::map(
                sequence::terminated(
                    bytes::tag("$"),
                    branch::alt((combinator::peek(bytes::is_not("*$")), combinator::eof)),
                ),
                |_| Wildcard::ZeroOrMore(Evaluation::Lazy).into(),
            ),
            combinator::map(
                sequence::delimited(
                    bytes::tag("["),
                    sequence::tuple((combinator::opt(bytes::tag("!")), archetypes)),
                    bytes::tag("]"),
                ),
                |(negation, archetypes)| {
                    Wildcard::Class {
                        is_negated: negation.is_some(),
                        archetypes,
                    }
                    .into()
                },
            ),
        ))(input)
    }

    fn alternative<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        sequence::delimited(
            bytes::tag("{"),
            combinator::map(
                multi::separated_list1(bytes::tag(","), glob),
                Token::Alternative,
            ),
            bytes::tag("}"),
        )(input)
    }

    fn separator<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        combinator::value(Token::NonTreeSeparator, bytes::tag("/"))(input)
    }

    fn glob<'i, E>(input: &'i str) -> IResult<&'i str, Vec<Token>, E>
    where
        E: ParseError<&'i str>,
    {
        multi::many1(branch::alt((literal, alternative, wildcard, separator)))(input)
    }

    combinator::all_consuming(glob)(text)
        .map(|(_, tokens)| tokens)
        .map_err(From::from)
}

pub fn optimize<'t>(
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
                    Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Eager)),
                    Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Eager))
                )
            )
        })
        .dedup_by(|left, right| {
            matches!(
                (left, right),
                (
                    Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Lazy)),
                    Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Lazy))
                )
            )
        })
        .coalesce(|left, right| match (&left, &right) {
            (
                Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Eager)),
                Token::Wildcard(Wildcard::ZeroOrMore(_)),
            )
            | (
                Token::Wildcard(Wildcard::ZeroOrMore(_)),
                Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Eager)),
            ) => Ok(Token::Wildcard(Wildcard::ZeroOrMore(Evaluation::Eager))),
            _ => Err((left, right)),
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
