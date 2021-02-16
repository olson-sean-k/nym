use filetime::FileTime;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::fs;
use std::num::ParseIntError;
use std::path::Path;
use std::str::{self, FromStr};

use crate::fmt::{self, Alignment, Format};
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
}

impl<'t> Format<'t> for Condition<'t> {
    fn format(&self, text: &'t str) -> Cow<'t, str> {
        match (text.is_empty(), &self.non_empty, &self.empty) {
            (true, _, Some(ref empty)) => empty.0.clone(),
            (false, Some(ref non_empty), _) => match non_empty {
                NonEmpty::Surround {
                    ref prefix,
                    ref postfix,
                } => format!("{}{}{}", prefix, text, postfix,).into(),
                NonEmpty::Literal(ref literal) => literal.clone(),
            },
            (true, _, None) | (false, None, _) => text.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct Substitution<'t> {
    subject: Subject<'t>,
    formatters: Vec<Formatter>,
}

impl<'t> Substitution<'t> {
    pub fn into_owned(self) -> Substitution<'static> {
        let Substitution {
            subject,
            formatters,
        } = self;
        Substitution {
            subject: subject.into_owned(),
            formatters,
        }
    }
}

#[derive(Clone, Debug)]
enum Subject<'t> {
    Capture(Capture<'t>),
    Property(Property),
}

impl<'t> Subject<'t> {
    pub fn into_owned(self) -> Subject<'static> {
        match self {
            Subject::Capture(capture) => capture.into_owned().into(),
            Subject::Property(property) => property.into(),
        }
    }
}

impl<'t> From<Capture<'t>> for Subject<'t> {
    fn from(capture: Capture<'t>) -> Self {
        Subject::Capture(capture)
    }
}

impl From<Property> for Subject<'static> {
    fn from(property: Property) -> Self {
        Subject::Property(property)
    }
}

#[derive(Clone, Debug)]
enum Formatter {
    Pad {
        shim: char,
        alignment: Alignment,
        width: usize,
    },
    Lower,
    Upper,
}

impl<'t> Format<'t> for Vec<Formatter> {
    fn format(&self, text: &'t str) -> Cow<'t, str> {
        if self.is_empty() {
            text.into()
        }
        else {
            let mut text = text.to_owned();
            for formatter in self {
                text = match *formatter {
                    Formatter::Pad {
                        shim,
                        alignment,
                        width,
                    } => fmt::pad(&text, shim, alignment, width).into_owned(),
                    Formatter::Lower => text.to_lowercase(),
                    Formatter::Upper => text.to_uppercase(),
                };
            }
            text.into()
        }
    }
}

#[derive(Clone, Debug)]
struct Capture<'a> {
    identifier: Identifier<'a>,
    condition: Option<Condition<'a>>,
}

impl<'a> Capture<'a> {
    pub fn into_owned(self) -> Capture<'static> {
        let Capture {
            identifier,
            condition,
        } = self;
        Capture {
            identifier: identifier.into_owned(),
            condition: condition.map(|condition| condition.into_owned()),
        }
    }
}

#[derive(Clone, Debug)]
enum Property {
    B3Sum,
    Timestamp,
}

#[derive(Clone, Debug)]
enum Token<'t> {
    Literal(Cow<'t, str>),
    Substitution(Substitution<'t>),
}

impl<'t> Token<'t> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Literal(literal) => literal.into_owned().into(),
            Token::Substitution(substitution) => substitution.into_owned().into(),
        }
    }
}

impl<'t> From<Substitution<'t>> for Token<'t> {
    fn from(substitution: Substitution<'t>) -> Self {
        Token::Substitution(substitution)
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

        fn bracketed<'i, O, E, F>(parser: F) -> impl FnMut(&'i str) -> IResult<&'i str, O, E>
        where
            E: ParseError<&'i str>,
            F: Parser<&'i str, O, E>,
        {
            sequence::delimited(character::char('['), parser, character::char(']'))
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
                        bracketed(branch::alt((bytes::is_not("]"), bytes::tag("")))),
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
                    bracketed(branch::alt((bytes::is_not("]"), bytes::tag("")))),
                    Cow::from,
                )(input)
            }

            fn non_empty<'i, E>(input: &'i str) -> IResult<&'i str, NonEmpty<'i>, E>
            where
                E: ParseError<&'i str>,
            {
                branch::alt((
                    combinator::map(
                        sequence::separated_pair(literal, bytes::tag(","), literal),
                        |(prefix, postfix)| NonEmpty::Surround { prefix, postfix },
                    ),
                    combinator::map(literal, NonEmpty::Literal),
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

        fn formatters<'i, E>(input: &'i str) -> IResult<&'i str, Vec<Formatter>, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            sequence::preceded(
                bytes::tag("|"),
                multi::separated_list0(
                    bytes::tag(","),
                    branch::alt((
                        combinator::map(
                            sequence::tuple((
                                branch::alt((
                                    combinator::value(Alignment::Left, bytes::tag("<")),
                                    combinator::value(Alignment::Center, bytes::tag("^")),
                                    combinator::value(Alignment::Right, bytes::tag(">")),
                                )),
                                combinator::map_res(character::digit1, |text: &'i str| {
                                    usize::from_str_radix(text, 10)
                                }),
                                bracketed(character::anychar),
                            )),
                            |(alignment, width, shim)| Formatter::Pad {
                                shim,
                                alignment,
                                width,
                            },
                        ),
                        combinator::value(Formatter::Lower, bytes::tag_no_case("l")),
                        combinator::value(Formatter::Upper, bytes::tag_no_case("u")),
                    )),
                ),
            )(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::map(
                braced(sequence::tuple((
                    identifier,
                    combinator::opt(condition),
                    branch::alt((formatters, combinator::success(Vec::new()))),
                ))),
                |(identifier, condition, formatters)| {
                    Token::from(Substitution {
                        subject: Subject::from(Capture {
                            identifier,
                            condition,
                        }),
                        formatters,
                    })
                },
            )(input)
        }

        fn property<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::map(
                braced(sequence::tuple((
                    sequence::preceded(
                        character::char('!'),
                        branch::alt((
                            combinator::map(bytes::tag_no_case("b3sum"), |_| Property::B3Sum),
                            combinator::map(bytes::tag_no_case("ts"), |_| Property::Timestamp),
                        )),
                    ),
                    branch::alt((formatters, combinator::success(Vec::new()))),
                ))),
                |(property, formatters)| {
                    Token::from(Substitution {
                        subject: Subject::from(property),
                        formatters,
                    })
                },
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
                Token::Substitution(Substitution {
                    ref subject,
                    ref formatters,
                }) => {
                    let text = match subject {
                        Subject::Capture(Capture {
                            ref identifier,
                            ref condition,
                        }) => {
                            let capture = match identifier {
                                Identifier::Index(ref index) => captures.get(ByIndex(*index)),
                                Identifier::Name(ref name) => captures.get(ByName(name)),
                            }
                            // Do not include empty captures. Captures that do
                            // not participate in a match and empty match text
                            // are treated the same way: the condition operates
                            // on an empty string.
                            .filter(|bytes| !bytes.is_empty())
                            .map(|bytes| str::from_utf8(bytes).map_err(PatternError::Encoding));
                            let capture: Cow<_> = if let Some(capture) = capture {
                                capture?.into()
                            }
                            else {
                                "".into()
                            };
                            if let Some(condition) = condition {
                                // TODO: `capture` does not live long enough to
                                //       escape this match arm, so the formatted
                                //       string must be copied. Restructuring
                                //       this code may avoid this copy.
                                condition.format(capture.as_ref()).into_owned()
                            }
                            else {
                                capture.into()
                            }
                        }
                        Subject::Property(ref property) => match *property {
                            Property::B3Sum => b3sum.get().map_err(PatternError::Property)?.into(),
                            Property::Timestamp => {
                                timestamp.get().map_err(PatternError::Property)?.into()
                            }
                        },
                    };
                    output.push_str(formatters.format(text.as_ref()).as_ref());
                }
                Token::Literal(ref text) => {
                    output.push_str(text);
                }
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
        ToPattern::parse("{#1?[prefix],[postfix]:}").unwrap();
        ToPattern::parse("{#1?:[no]}").unwrap();
        ToPattern::parse("{#1?[],[-]:[no]}").unwrap();
    }

    #[test]
    fn parse_to_pattern_formatter() {
        ToPattern::parse("{#1|>4[0]}").unwrap();
        ToPattern::parse("{#1|u}").unwrap();
        ToPattern::parse("{#1|<2[ ],l}").unwrap();
    }

    #[test]
    fn reject_to_pattern_with_empty_case_surround() {
        assert!(ToPattern::parse("{#1?:[prefix],[postfix]}").is_err());
    }

    #[test]
    fn reject_to_pattern_out_of_order() {
        assert!(ToPattern::parse("{#1|u?:}").is_err());
    }
}
