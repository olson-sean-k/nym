use std::borrow::Cow;
use std::num::ParseIntError;

use crate::fmt::Alignment;
use crate::pattern::PatternError;

#[derive(Clone, Debug)]
pub enum Identifier<'t> {
    Index(usize),
    Name(Cow<'t, str>),
}

impl<'t> Identifier<'t> {
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

impl<'t> From<Cow<'t, str>> for Identifier<'t> {
    fn from(name: Cow<'t, str>) -> Self {
        Identifier::Name(name)
    }
}

impl<'t> From<&'t str> for Identifier<'t> {
    fn from(name: &'t str) -> Self {
        Identifier::Name(name.into())
    }
}

impl From<String> for Identifier<'static> {
    fn from(name: String) -> Self {
        Identifier::Name(name.into())
    }
}

#[derive(Clone, Debug)]
pub enum NonEmptyCase<'t> {
    Surround {
        prefix: Cow<'t, str>,
        postfix: Cow<'t, str>,
    },
    Literal(Cow<'t, str>),
}

impl<'t> NonEmptyCase<'t> {
    pub fn into_owned(self) -> NonEmptyCase<'static> {
        match self {
            NonEmptyCase::Surround { prefix, postfix } => NonEmptyCase::Surround {
                prefix: prefix.into_owned().into(),
                postfix: postfix.into_owned().into(),
            },
            NonEmptyCase::Literal(literal) => NonEmptyCase::Literal(literal.into_owned().into()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct EmptyCase<'t>(pub Cow<'t, str>);

impl<'t> EmptyCase<'t> {
    pub fn into_owned(self) -> EmptyCase<'static> {
        let EmptyCase(literal) = self;
        EmptyCase(literal.into_owned().into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct Condition<'t> {
    pub non_empty: Option<NonEmptyCase<'t>>,
    pub empty: Option<EmptyCase<'t>>,
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

#[derive(Clone, Debug)]
pub struct Substitution<'t> {
    pub subject: Subject<'t>,
    pub formatters: Vec<Formatter>,
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
pub enum Subject<'t> {
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
pub enum Formatter {
    Pad {
        shim: char,
        alignment: Alignment,
        width: usize,
    },
    Lower,
    Upper,
}

#[derive(Clone, Debug)]
pub struct Capture<'t> {
    pub identifier: Identifier<'t>,
    pub condition: Option<Condition<'t>>,
}

impl<'t> Capture<'t> {
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
pub enum Property {
    B3Sum,
    Timestamp,
}

#[derive(Clone, Debug)]
pub enum Token<'t> {
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

pub fn parse(text: &str) -> Result<Vec<Token>, PatternError> {
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

    fn escaped<'i, E, F>(parser: F) -> impl FnMut(&'i str) -> IResult<&'i str, String, E>
    where
        E: ParseError<&'i str>,
        F: Parser<&'i str, &'i str, E>,
    {
        combinator::verify(
            bytes::escaped_transform(
                parser,
                '\\',
                branch::alt((
                    combinator::value("[", bytes::tag("[")),
                    combinator::value("]", bytes::tag("]")),
                    combinator::value("{", bytes::tag("{")),
                    combinator::value("}", bytes::tag("}")),
                    combinator::value("\\", bytes::tag("\\")),
                )),
            ),
            |text: &str| !text.is_empty(),
        )
    }

    fn argument<'i, E>(input: &'i str) -> IResult<&'i str, Cow<'i, str>, E>
    where
        E: ParseError<&'i str>,
    {
        bracketed(branch::alt((
            combinator::map(escaped(bytes::is_not("[]\\")), Cow::from),
            combinator::map(bytes::tag(""), Cow::from),
        )))(input)
    }

    fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: ParseError<&'i str>,
    {
        combinator::map(escaped(bytes::is_not("{}\\")), Token::from)(input)
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
                sequence::preceded(character::char('@'), argument),
                Identifier::from,
            ),
            combinator::value(Identifier::from(0), character::space0),
        ))(input)
    }

    fn condition<'i, E>(input: &'i str) -> IResult<&'i str, Condition, E>
    where
        E: ParseError<&'i str>,
    {
        fn non_empty<'i, E>(input: &'i str) -> IResult<&'i str, NonEmptyCase<'i>, E>
        where
            E: ParseError<&'i str>,
        {
            branch::alt((
                combinator::map(
                    sequence::separated_pair(argument, bytes::tag(","), argument),
                    |(prefix, postfix)| NonEmptyCase::Surround { prefix, postfix },
                ),
                combinator::map(argument, NonEmptyCase::Literal),
            ))(input)
        }

        combinator::map(
            sequence::preceded(
                bytes::tag("?"),
                sequence::separated_pair(
                    combinator::opt(non_empty),
                    bytes::tag(":"),
                    combinator::opt(combinator::map(argument, EmptyCase)),
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
                            bracketed(branch::alt((
                                character::none_of("[]\\"),
                                branch::alt((
                                    combinator::value('[', bytes::tag("\\[")),
                                    combinator::value(']', bytes::tag("\\]")),
                                    combinator::value('\\', bytes::tag("\\\\")),
                                )),
                            ))),
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

    combinator::all_consuming(multi::many1(branch::alt((literal, capture, property))))(text)
        .map(|(_, tokens)| tokens)
        .map_err(From::from)
}
