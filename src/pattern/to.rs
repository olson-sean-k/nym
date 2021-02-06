use filetime::FileTime;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::fs;
use std::num::ParseIntError;
use std::path::Path;
use std::str::{self, FromStr};

use crate::glob::{ByIndex, ByName, Captures};
use crate::pattern::PatternError;

#[derive(Clone, Debug)]
enum Identifier<'a> {
    Index(usize),
    Name(Cow<'a, str>),
}

impl<'a> Identifier<'a> {
    pub fn into_owned(self) -> Identifier<'static> {
        match self {
            Identifier::Index(index) => index.into(),
            Identifier::Name(name) => name.into_owned().into(),
        }
    }
}

impl From<usize> for Identifier<'static> {
    fn from(index: usize) -> Self {
        Identifier::Index(index)
    }
}

impl<'a> From<&'a str> for Identifier<'a> {
    fn from(name: &'a str) -> Self {
        Identifier::Name(Cow::Borrowed(name))
    }
}

impl From<String> for Identifier<'static> {
    fn from(name: String) -> Self {
        Identifier::Name(Cow::Owned(name))
    }
}

#[derive(Clone, Debug)]
struct Substitution<'a> {
    prefix: Cow<'a, str>,
    postfix: Cow<'a, str>,
    absent: Cow<'a, str>,
}

impl<'a> Substitution<'a> {
    pub fn into_owned(self) -> Substitution<'static> {
        let Substitution {
            prefix,
            postfix,
            absent,
        } = self;
        Substitution {
            prefix: prefix.into_owned().into(),
            postfix: postfix.into_owned().into(),
            absent: absent.into_owned().into(),
        }
    }

    pub fn format<'t>(&self, capture: &'t str) -> Cow<'t, str> {
        if self.prefix.is_empty() && self.postfix.is_empty() {
            capture.into()
        }
        else {
            format!("{}{}{}", self.prefix, capture, self.postfix).into()
        }
    }
}

#[derive(Clone, Debug)]
struct Capture<'a> {
    identifier: Identifier<'a>,
    substitution: Option<Substitution<'a>>,
}

impl<'a> Capture<'a> {
    pub fn into_owned(self) -> Capture<'static> {
        let Capture {
            identifier,
            substitution,
        } = self;
        Capture {
            identifier: identifier.into_owned(),
            substitution: substitution.map(|substitution| substitution.into_owned()),
        }
    }
}

#[derive(Clone, Debug)]
enum Property {
    B3Sum,
    Timestamp,
}

#[derive(Clone, Debug)]
enum Token<'a> {
    Capture(Capture<'a>),
    Literal(Cow<'a, str>),
    Property(Property),
}

impl<'a> Token<'a> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Capture(capture) => capture.into_owned().into(),
            Token::Literal(literal) => literal.into_owned().into(),
            Token::Property(property) => Token::Property(property),
        }
    }
}

impl<'a> From<Capture<'a>> for Token<'a> {
    fn from(capture: Capture<'a>) -> Self {
        Token::Capture(capture)
    }
}

impl<'a> From<&'a str> for Token<'a> {
    fn from(literal: &'a str) -> Self {
        Token::Literal(Cow::Borrowed(literal))
    }
}

impl From<String> for Token<'static> {
    fn from(literal: String) -> Self {
        Token::Literal(Cow::Owned(literal))
    }
}

#[derive(Clone, Debug)]
pub struct ToPattern<'a> {
    tokens: Vec<Token<'a>>,
}

impl<'a> ToPattern<'a> {
    pub fn parse(text: &'a str) -> Result<Self, PatternError> {
        use nom::bytes::complete as bytes;
        use nom::character::complete as character;
        use nom::error::{FromExternalError, ParseError};
        use nom::{branch, combinator, multi, sequence, IResult, Parser};

        fn braced<'i, O, E, F>(parser: F) -> impl FnMut(&'i str) -> IResult<&'i str, O, E>
        where
            E: ParseError<&'i str>,
            F: Parser<&'i str, O, E>,
        {
            sequence::delimited(character::char('{'), parser, character::char('}'))
        }

        // TODO: Support escaping captures.
        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(bytes::is_not("{"), From::from)(input)
        }

        fn identifier<'i, E>(input: &'i str) -> IResult<&'i str, Identifier, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            branch::alt((
                combinator::map_res(
                    sequence::preceded(character::char('#'), character::digit1),
                    |text: &'i str| usize::from_str_radix(text, 10).map(Identifier::from),
                ),
                combinator::map(
                    sequence::preceded(
                        character::char('@'),
                        // TODO: `regex` supports additional name characters.
                        character::alphanumeric1,
                    ),
                    Identifier::from,
                ),
                combinator::value(Identifier::from(0), character::space0),
            ))(input)
        }

        fn substitution<'i, E>(input: &'i str) -> IResult<&'i str, Substitution, E>
        where
            E: ParseError<&'i str>,
        {
            let element = |input| {
                combinator::opt(combinator::map(
                    // TODO: Stopping at `:}` is probably clunky. Note that
                    //       `character::alphanumeric0` or similar categorical
                    //       parsers may not work well, since they cannot
                    //       support non-ASCII characters.
                    bytes::is_not(":}"),
                    Cow::from,
                ))(input)
            };
            combinator::map(
                sequence::separated_pair(
                    element,
                    bytes::tag(":"),
                    sequence::separated_pair(element, bytes::tag(":"), element),
                ),
                |(prefix, (postfix, absent))| Substitution {
                    prefix: prefix.unwrap_or_else(|| "".into()),
                    postfix: postfix.unwrap_or_else(|| "".into()),
                    absent: absent.unwrap_or_else(|| "".into()),
                },
            )(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::map(
                braced(sequence::tuple((
                    identifier,
                    combinator::opt(sequence::preceded(bytes::tag("?"), substitution)),
                ))),
                |(identifier, substitution)| {
                    Token::from(Capture {
                        identifier,
                        substitution,
                    })
                },
            )(input)
        }

        fn property<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            braced(sequence::preceded(
                character::char('!'),
                branch::alt((
                    combinator::map(bytes::tag_no_case("b3sum"), |_| {
                        Token::Property(Property::B3Sum)
                    }),
                    combinator::map(bytes::tag_no_case("ts"), |_| {
                        Token::Property(Property::Timestamp)
                    }),
                )),
            ))(input)
        }

        fn pattern<'i, E>(input: &'i str) -> IResult<&'i str, ToPattern, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map(
                multi::many1(branch::alt((literal, capture, property))),
                move |tokens| ToPattern { tokens },
            ))(input)
        }

        pattern::<(_, ErrorKind)>(text)
            .map(|(_, pattern)| pattern)
            .map_err(Into::into)
    }

    pub fn into_owned(self) -> ToPattern<'static> {
        let ToPattern { tokens } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        ToPattern { tokens }
    }

    pub fn resolve(
        &self,
        source: impl AsRef<Path>,
        captures: &Captures<'_>,
    ) -> Result<String, PatternError> {
        let mut output = String::new();
        for token in &self.tokens {
            match *token {
                Token::Capture(Capture {
                    ref identifier,
                    ref substitution,
                }) => {
                    let capture = match identifier {
                        Identifier::Index(ref index) => captures.get(ByIndex(*index)),
                        Identifier::Name(ref name) => captures.get(ByName(name)),
                    }
                    // Do not include empty captures. This means that absent
                    // substitutions are applied when a capture technically
                    // participates in a match but with no bytes. This typically
                    // occurs when using Kleene stars.
                    .filter(|bytes| !bytes.is_empty())
                    .map(|bytes| str::from_utf8(bytes).map_err(PatternError::Encoding));
                    let text: Cow<_> = if let Some(capture) = capture {
                        let capture = capture?;
                        if let Some(substitution) = substitution {
                            substitution.format(capture)
                        }
                        else {
                            capture.into()
                        }
                    }
                    else {
                        // TODO: If there is no substitution here, a
                        //       `CaptureNotFound` error could be emitted, but
                        //       an empty string is a reasonable output.
                        //       Moreover, the `regex` crate does not provide a
                        //       way to distinguish between the existence of a
                        //       capture in an expression vs. its participation
                        //       in a match. Is there some way to reconcile
                        //       this?
                        substitution
                            .as_ref()
                            .map(|substitution| substitution.absent.clone())
                            .unwrap_or_else(|| "".into())
                    };
                    output.push_str(text.as_ref());
                }
                Token::Literal(ref text) => {
                    output.push_str(text);
                }
                Token::Property(ref property) => match *property {
                    Property::B3Sum => {
                        let hash = blake3::hash(
                            fs::read(source.as_ref())
                                .map_err(PatternError::Property)?
                                .as_ref(),
                        );
                        output.push_str(hash.to_hex().as_str());
                    }
                    Property::Timestamp => {
                        let metadata =
                            fs::metadata(source.as_ref()).map_err(PatternError::Property)?;
                        let time = FileTime::from_last_modification_time(&metadata);
                        output.push_str(&format!("{}", time));
                    }
                },
            }
        }
        Ok(output)
    }
}

impl FromStr for ToPattern<'static> {
    type Err = PatternError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        ToPattern::parse(text).map(|pattern| pattern.into_owned())
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::ToPattern;

    #[test]
    fn parse_to_pattern_substitution() {
        ToPattern::parse("{#1?a:b:c}").unwrap();
        ToPattern::parse("{#1?a:b:}").unwrap();
        ToPattern::parse("{#1?a::}").unwrap();
        ToPattern::parse("{#1?::}").unwrap();
    }
}
