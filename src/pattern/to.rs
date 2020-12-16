use filetime::FileTime;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::fs;
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
    Hash,
    Timestamp,
}

#[derive(Clone, Debug)]
enum Component<'a> {
    Capture(Capture<'a>),
    Literal(Cow<'a, str>),
    Property(Property),
}

impl<'a> Component<'a> {
    pub fn into_owned(self) -> Component<'static> {
        match self {
            Component::Capture(capture) => capture.into_owned().into(),
            Component::Literal(literal) => literal.into_owned().into(),
            Component::Property(property) => Component::Property(property),
        }
    }
}

impl<'a> From<Capture<'a>> for Component<'a> {
    fn from(capture: Capture<'a>) -> Self {
        Component::Capture(capture)
    }
}

impl<'a> From<&'a str> for Component<'a> {
    fn from(literal: &'a str) -> Self {
        Component::Literal(Cow::Borrowed(literal))
    }
}

impl From<String> for Component<'static> {
    fn from(literal: String) -> Self {
        Component::Literal(Cow::Owned(literal))
    }
}

#[derive(Clone, Debug)]
pub struct ToPattern<'a> {
    components: Vec<Component<'a>>,
}

impl<'a> ToPattern<'a> {
    pub fn into_owned(self) -> ToPattern<'static> {
        let ToPattern { components } = self;
        let components = components
            .into_iter()
            .map(|component| component.into_owned())
            .collect();
        ToPattern { components }
    }

    pub fn resolve(
        &self,
        source: impl AsRef<Path>,
        find: &Find<'_>,
    ) -> Result<String, PatternError> {
        let mut output = String::new();
        for component in &self.components {
            match *component {
                Component::Capture(ref capture) => match capture {
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
                Component::Literal(ref text) => {
                    output.push_str(text);
                }
                Component::Property(ref property) => match *property {
                    Property::Hash => {
                        let hash = blake3::hash(fs::read(source.as_ref())?.as_ref());
                        output.push_str(hash.to_hex().as_str());
                    }
                    Property::Timestamp => {
                        let metadata = fs::metadata(source.as_ref())?;
                        let time = FileTime::from_last_modification_time(&metadata);
                        output.push_str(&format!("{}", time));
                    }
                },
            }
        }
        Ok(output)
    }
}

impl<'a> ToPattern<'a> {
    pub fn parse(text: &'a str) -> Result<Self, PatternError> {
        use nom::bytes::complete as bytes;
        use nom::character::complete as character;
        use nom::error::ParseError;
        use nom::{branch, combinator, multi, sequence, IResult};

        // TODO: Support escaping captures.
        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(bytes::is_not("{"), From::from)(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            let index = |text: &'i str| {
                usize::from_str_radix(text, 10)
                    .map(|index| Component::from(Capture::from(index)))
                    .ok()
            };
            let name = |text: &'i str| Some(Component::from(Capture::from(text)));
            sequence::delimited(
                character::char('{'),
                branch::alt((
                    // TODO: Support empty braces. Note that using `space0`
                    //       conflicts with the alternate parsers.
                    combinator::value(Component::from(Capture::from(0)), character::space1),
                    combinator::map_opt(
                        sequence::preceded(character::char('#'), character::digit1),
                        index,
                    ),
                    combinator::map_opt(
                        sequence::preceded(
                            character::char('@'),
                            // TODO: `regex` supports additional name characters.
                            character::alphanumeric1,
                        ),
                        name,
                    ),
                )),
                character::char('}'),
            )(input)
        }

        fn property<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            sequence::delimited(
                character::char('{'),
                sequence::preceded(
                    character::char('!'),
                    branch::alt((
                        combinator::map(bytes::tag_no_case("hash"), |_| {
                            Component::Property(Property::Hash)
                        }),
                        combinator::map(bytes::tag_no_case("timestamp"), |_| {
                            Component::Property(Property::Timestamp)
                        }),
                    )),
                ),
                character::char('}'),
            )(input)
        }

        fn pattern<'i, E>(input: &'i str) -> IResult<&'i str, ToPattern, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map(
                multi::many1(branch::alt((literal, capture, property))),
                move |components| ToPattern { components },
            ))(input)
        }

        pattern::<(_, ErrorKind)>(text)
            .map(|(_, pattern)| pattern)
            .map_err(Into::into)
    }
}

impl FromStr for ToPattern<'static> {
    type Err = PatternError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        ToPattern::parse(text).map(|pattern| pattern.into_owned())
    }
}
