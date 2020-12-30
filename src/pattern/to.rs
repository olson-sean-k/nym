use filetime::FileTime;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::fs;
use std::num::ParseIntError;
use std::path::Path;
use std::str::FromStr;

use crate::pattern::from::{Find, Selector};
use crate::pattern::PatternError;

use Selector::ByIndex;
use Selector::ByName;

#[derive(Clone, Debug)]
enum Capture<'a> {
    Index(usize),
    Name(Cow<'a, str>),
}

impl<'a> Capture<'a> {
    pub fn into_owned(self) -> Capture<'static> {
        match self {
            Capture::Index(index) => index.into(),
            Capture::Name(name) => name.into_owned().into(),
        }
    }
}

impl From<usize> for Capture<'static> {
    fn from(index: usize) -> Self {
        Capture::Index(index)
    }
}

impl<'a> From<&'a str> for Capture<'a> {
    fn from(name: &'a str) -> Self {
        Capture::Name(Cow::Borrowed(name))
    }
}

impl From<String> for Capture<'static> {
    fn from(name: String) -> Self {
        Capture::Name(Cow::Owned(name))
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
        use nom::{branch, combinator, multi, sequence, IResult};

        // TODO: Support escaping captures.
        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(bytes::is_not("{"), From::from)(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            sequence::delimited(
                character::char('{'),
                branch::alt((
                    // TODO: Support empty braces. Note that using `space0`
                    //       conflicts with the alternate parsers.
                    combinator::value(Token::from(Capture::from(0)), character::space1),
                    combinator::map_res(
                        sequence::preceded(character::char('#'), character::digit1),
                        |text: &'i str| {
                            usize::from_str_radix(text, 10)
                                .map(|index| Token::from(Capture::from(index)))
                        },
                    ),
                    combinator::map(
                        sequence::preceded(
                            character::char('@'),
                            // TODO: `regex` supports additional name characters.
                            character::alphanumeric1,
                        ),
                        |text: &'i str| Token::from(Capture::from(text)),
                    ),
                )),
                character::char('}'),
            )(input)
        }

        fn property<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            sequence::delimited(
                character::char('{'),
                sequence::preceded(
                    character::char('!'),
                    branch::alt((
                        combinator::map(bytes::tag_no_case("b3sum"), |_| {
                            Token::Property(Property::B3Sum)
                        }),
                        combinator::map(bytes::tag_no_case("ts"), |_| {
                            Token::Property(Property::Timestamp)
                        }),
                    )),
                ),
                character::char('}'),
            )(input)
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
        find: &Find<'_>,
    ) -> Result<String, PatternError> {
        let mut output = String::new();
        for token in &self.tokens {
            match *token {
                Token::Capture(ref capture) => match capture {
                    Capture::Index(ref index) => {
                        output.push_str(
                            find.capture(ByIndex(*index))
                                .ok_or(PatternError::CaptureNotFound)?,
                        );
                    }
                    Capture::Name(ref name) => {
                        output.push_str(
                            find.capture(ByName(name))
                                .ok_or(PatternError::CaptureNotFound)?,
                        );
                    }
                },
                Token::Literal(ref text) => {
                    output.push_str(text);
                }
                Token::Property(ref property) => match *property {
                    Property::B3Sum => {
                        let hash = blake3::hash(
                            fs::read(source.as_ref())
                                .map_err(|error| PatternError::ReadProperty(error))?
                                .as_ref(),
                        );
                        output.push_str(hash.to_hex().as_str());
                    }
                    Property::Timestamp => {
                        let metadata = fs::metadata(source.as_ref())
                            .map_err(|error| PatternError::ReadProperty(error))?;
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
