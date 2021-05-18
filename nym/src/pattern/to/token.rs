use chrono::{DateTime, TimeZone};
use smallvec::SmallVec;
use std::borrow::Cow;
use std::fmt::Display;
use std::num::ParseIntError;

use crate::pattern::PatternError;
use crate::text::Alignment;

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
    pub formatters: Vec<TextFormatter>,
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
    Property(Property<'t>),
}

impl<'t> Subject<'t> {
    pub fn into_owned(self) -> Subject<'static> {
        match self {
            Subject::Capture(capture) => capture.into_owned().into(),
            Subject::Property(property) => property.into_owned().into(),
        }
    }
}

impl<'t> From<Capture<'t>> for Subject<'t> {
    fn from(capture: Capture<'t>) -> Self {
        Subject::Capture(capture)
    }
}

impl<'t> From<Property<'t>> for Subject<'t> {
    fn from(property: Property<'t>) -> Self {
        Subject::Property(property)
    }
}

#[derive(Clone, Debug)]
pub enum TextFormatter {
    Coalesce {
        from: SmallVec<[char; 4]>,
        to: char,
    },
    Pad {
        shim: char,
        alignment: Alignment,
        width: usize,
    },
    Lower,
    Title,
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

pub trait PropertyFormat<M> {
    fn fmt(&self, fmt: &M) -> String;
}

// Numeric formats that include alphabetic characters are always lowercase where
// applicable.
#[derive(Clone, Copy, Debug)]
pub enum DigestFormat {
    Hexadecimal,
}

impl Default for DigestFormat {
    fn default() -> Self {
        DigestFormat::Hexadecimal
    }
}

#[cfg(feature = "property-b3sum")]
impl PropertyFormat<DigestFormat> for blake3::Hash {
    fn fmt(&self, fmt: &DigestFormat) -> String {
        match fmt {
            DigestFormat::Hexadecimal => self.to_hex().as_str().to_owned(),
        }
    }
}

#[cfg(feature = "property-md5sum")]
impl PropertyFormat<DigestFormat> for md5::Digest {
    fn fmt(&self, fmt: &DigestFormat) -> String {
        match fmt {
            DigestFormat::Hexadecimal => format!("{:x}", self),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DateTimeFormat<'t> {
    fmt: Cow<'t, str>,
}

impl<'t> DateTimeFormat<'t> {
    pub fn into_owned(self) -> DateTimeFormat<'static> {
        DateTimeFormat {
            fmt: self.fmt.into_owned().into(),
        }
    }
}

impl<'t> Default for DateTimeFormat<'t> {
    fn default() -> Self {
        DateTimeFormat {
            fmt: "%F-%X".into(),
        }
    }
}

impl<'t> From<Cow<'t, str>> for DateTimeFormat<'t> {
    fn from(fmt: Cow<'t, str>) -> Self {
        DateTimeFormat { fmt }
    }
}

impl<'t, Z> PropertyFormat<DateTimeFormat<'t>> for DateTime<Z>
where
    Z: TimeZone,
    Z::Offset: Display,
{
    fn fmt(&self, fmt: &DateTimeFormat<'t>) -> String {
        self.format(fmt.fmt.as_ref()).to_string()
    }
}

#[derive(Clone, Debug)]
pub enum Property<'t> {
    #[cfg(feature = "property-b3sum")]
    B3Sum(DigestFormat),
    CTime(DateTimeFormat<'t>),
    #[cfg(feature = "property-md5sum")]
    Md5Sum(DigestFormat),
    MTime(DateTimeFormat<'t>),
}

impl<'t> Property<'t> {
    pub fn into_owned(self) -> Property<'static> {
        match self {
            #[cfg(feature = "property-b3sum")]
            Property::B3Sum(fmt) => Property::B3Sum(fmt),
            Property::CTime(fmt) => Property::CTime(fmt.into_owned()),
            #[cfg(feature = "property-md5sum")]
            Property::Md5Sum(fmt) => Property::Md5Sum(fmt),
            Property::MTime(fmt) => Property::MTime(fmt.into_owned()),
        }
    }
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

    /// Parses an argument.
    ///
    /// An argument is arbitrary text delimited by square brackets. Within an
    /// argument, square brackets may be escaped with a back slash.
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
                |text: &'i str| text.parse::<usize>().map(Identifier::from),
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

    /// Parses a sequence of text formatters.
    fn formatters<'i, E>(input: &'i str) -> IResult<&'i str, Vec<TextFormatter>, E>
    where
        E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
    {
        sequence::preceded(
            bytes::tag("|"),
            multi::separated_list0(
                bytes::tag(","),
                branch::alt((
                    combinator::map(
                        sequence::preceded(
                            bytes::tag("%"),
                            sequence::tuple((
                                argument,
                                bracketed(branch::alt((
                                    character::none_of("[]\\"),
                                    branch::alt((
                                        combinator::value('[', bytes::tag("\\[")),
                                        combinator::value(']', bytes::tag("\\]")),
                                        combinator::value('\\', bytes::tag("\\\\")),
                                    )),
                                ))),
                            )),
                        ),
                        |(from, to)| TextFormatter::Coalesce {
                            from: from.chars().collect(),
                            to,
                        },
                    ),
                    combinator::map(
                        sequence::tuple((
                            branch::alt((
                                combinator::value(Alignment::Left, bytes::tag("<")),
                                combinator::value(Alignment::Center, bytes::tag("^")),
                                combinator::value(Alignment::Right, bytes::tag(">")),
                            )),
                            combinator::map_res(character::digit1, |text: &'i str| {
                                text.parse::<usize>()
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
                        |(alignment, width, shim)| TextFormatter::Pad {
                            shim,
                            alignment,
                            width,
                        },
                    ),
                    combinator::value(TextFormatter::Lower, bytes::tag_no_case("lower")),
                    combinator::value(TextFormatter::Title, bytes::tag_no_case("title")),
                    combinator::value(TextFormatter::Upper, bytes::tag_no_case("upper")),
                )),
            ),
        )(input)
    }

    /// Parses a capture substition (identifier, condition, and text
    /// formatters).
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

    /// Parses a property substitution (property format and text formatters).
    fn property<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
    where
        E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
    {
        /// Parses a property format that can be constructed from argument text.
        fn fmt_from_str<'i, T, E>(input: &'i str) -> IResult<&'i str, T, E>
        where
            T: Default + From<Cow<'i, str>>,
            E: FromExternalError<&'i str, ParseIntError> + ParseError<&'i str>,
        {
            combinator::map(
                combinator::opt(sequence::preceded(bytes::tag(":"), argument)),
                |text| text.map(T::from).unwrap_or_default(),
            )(input)
        }

        combinator::map(
            braced(sequence::tuple((
                sequence::preceded(
                    character::char('!'),
                    branch::alt((
                        #[cfg(feature = "property-b3sum")]
                        combinator::map(bytes::tag_no_case("b3sum"), |_| {
                            Property::B3Sum(Default::default())
                        }),
                        sequence::preceded(
                            bytes::tag_no_case("ctime"),
                            combinator::map(fmt_from_str, Property::CTime),
                        ),
                        #[cfg(feature = "property-md5sum")]
                        combinator::map(bytes::tag_no_case("md5sum"), |_| {
                            Property::Md5Sum(Default::default())
                        }),
                        sequence::preceded(
                            bytes::tag_no_case("mtime"),
                            combinator::map(fmt_from_str, Property::MTime),
                        ),
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
