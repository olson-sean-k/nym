use filetime::FileTime;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::fs;
use std::num::ParseIntError;
use std::path::Path;
use std::str::{self, FromStr};

use crate::glob::{ByIndex, ByName, Captures};
use crate::memoize::Memoized;
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
enum NonEmpty<'t> {
    Surround {
        prefix: Cow<'t, str>,
        postfix: Cow<'t, str>,
    },
    Literal(Cow<'t, str>),
}

impl<'t> NonEmpty<'t> {
    pub fn into_owned(self) -> NonEmpty<'static> {
        match self {
            NonEmpty::Surround { prefix, postfix } => NonEmpty::Surround {
                prefix: prefix.into_owned().into(),
                postfix: postfix.into_owned().into(),
            },
            NonEmpty::Literal(literal) => NonEmpty::Literal(literal.into_owned().into()),
        }
    }
}

#[derive(Clone, Debug)]
struct Empty<'t>(Cow<'t, str>);

impl<'t> Empty<'t> {
    pub fn into_owned(self) -> Empty<'static> {
        let Empty(literal) = self;
        Empty(literal.into_owned().into())
    }
}

#[derive(Clone, Debug, Default)]
struct Condition<'t> {
    non_empty: Option<NonEmpty<'t>>,
    empty: Option<Empty<'t>>,
}

impl<'t> Condition<'t> {
    pub fn into_owned(self) -> Condition<'static> {
        let Condition { non_empty, empty } = self;
        Condition {
            non_empty: non_empty.map(|non_empty| non_empty.into_owned()),
            empty: empty.map(|empty| empty.into_owned()),
        }
    }

    pub fn format(&self, capture: &'t str) -> Cow<'t, str> {
        match (capture.is_empty(), &self.non_empty, &self.empty) {
            (true, _, Some(ref empty)) => empty.0.clone(),
            (false, Some(ref non_empty), _) => match non_empty {
                NonEmpty::Surround {
                    ref prefix,
                    ref postfix,
                } => format!("{}{}{}", prefix, capture, postfix,).into(),
                NonEmpty::Literal(ref literal) => literal.clone(),
            },
            (true, _, None) | (false, None, _) => capture.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct Capture<'a> {
    identifier: Identifier<'a>,
    condition: Condition<'a>,
}

impl<'a> Capture<'a> {
    pub fn into_owned(self) -> Capture<'static> {
        let Capture {
            identifier,
            condition,
        } = self;
        Capture {
            identifier: identifier.into_owned(),
            condition: condition.into_owned(),
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

        fn condition<'i, E>(input: &'i str) -> IResult<&'i str, Condition, E>
        where
            E: ParseError<&'i str>,
        {
            fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Cow<'i, str>, E>
            where
                E: ParseError<&'i str>,
            {
                combinator::map(
                    sequence::delimited(
                        bytes::tag("["),
                        branch::alt((bytes::is_not("]"), bytes::tag(""))),
                        bytes::tag("]"),
                    ),
                    Cow::from,
                )(input)
            }

            fn non_empty<'i, E>(input: &'i str) -> IResult<&'i str, NonEmpty<'i>, E>
            where
                E: ParseError<&'i str>,
            {
                let element = |input| {
                    combinator::map(
                        branch::alt((bytes::is_not(",)"), bytes::tag(""))),
                        Cow::from,
                    )(input)
                };
                branch::alt((
                    combinator::map(literal, NonEmpty::Literal),
                    combinator::map(
                        sequence::delimited(
                            bytes::tag("("),
                            sequence::separated_pair(element, bytes::tag(","), element),
                            bytes::tag(")"),
                        ),
                        |(prefix, postfix)| NonEmpty::Surround { prefix, postfix },
                    ),
                ))(input)
            }

            combinator::map(
                sequence::preceded(
                    bytes::tag("?"),
                    sequence::separated_pair(
                        combinator::opt(non_empty),
                        bytes::tag(":"),
                        combinator::opt(combinator::map(literal, Empty)),
                    ),
                ),
                |(non_empty, empty)| Condition { non_empty, empty },
            )(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::map(
                braced(sequence::tuple((
                    identifier,
                    branch::alt((condition, combinator::success(Condition::default()))),
                ))),
                |(identifier, condition)| {
                    Token::from(Capture {
                        identifier,
                        condition,
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
        let mut b3sum = Memoized::from(|| {
            fs::read(source.as_ref())
                .map(|data| blake3::hash(data.as_ref()).to_hex().as_str().to_owned())
        });
        let mut timestamp = Memoized::from(|| {
            fs::metadata(source.as_ref())
                .map(|metadata| format!("{}", FileTime::from_last_modification_time(&metadata)))
        });
        let mut output = String::new();
        for token in &self.tokens {
            match *token {
                Token::Capture(Capture {
                    ref identifier,
                    ref condition,
                }) => {
                    let capture = match identifier {
                        Identifier::Index(ref index) => captures.get(ByIndex(*index)),
                        Identifier::Name(ref name) => captures.get(ByName(name)),
                    }
                    // Do not include empty captures. Captures that do not
                    // participate in a match and empty match text are treated
                    // the same way: the condition operates on an empty string.
                    .filter(|bytes| !bytes.is_empty())
                    .map(|bytes| str::from_utf8(bytes).map_err(PatternError::Encoding));
                    let capture: Cow<_> = if let Some(capture) = capture {
                        capture?.into()
                    }
                    else {
                        "".into()
                    };
                    output.push_str(condition.format(capture.as_ref()).as_ref());
                }
                Token::Literal(ref text) => {
                    output.push_str(text);
                }
                Token::Property(ref property) => match *property {
                    Property::B3Sum => {
                        output.push_str(b3sum.get().map_err(PatternError::Property)?);
                    }
                    Property::Timestamp => {
                        output.push_str(timestamp.get().map_err(PatternError::Property)?);
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
    fn parse_to_pattern() {
        ToPattern::parse("{}").unwrap();
        ToPattern::parse("{#1}").unwrap();
        ToPattern::parse("literal{#1}").unwrap();
        ToPattern::parse("{#1}literal").unwrap();
    }

    #[test]
    fn parse_to_pattern_condition() {
        ToPattern::parse("{#1?:}").unwrap();
        ToPattern::parse("{#1?[yes]:}").unwrap();
        ToPattern::parse("{#1?[]:}").unwrap();
        ToPattern::parse("{#1?(prefix,postfix):}").unwrap();
        ToPattern::parse("{#1?:[no]}").unwrap();
        ToPattern::parse("{#1?(,-):[no]}").unwrap();
    }

    #[test]
    fn reject_to_pattern_with_empty_case_surround() {
        assert!(ToPattern::parse("{#1?:(prefix,postfix)}").is_err());
    }
}
