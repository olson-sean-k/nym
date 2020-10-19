pub mod copy;
pub mod edit;
pub mod r#move;

use nom;
use nom::error::ErrorKind;
use std::borrow::Cow;
use std::str::FromStr;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum TransformError {
    #[error("failed to parse to-pattern")]
    PatternParse,
}

impl<I> From<nom::Err<(I, ErrorKind)>> for TransformError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        TransformError::PatternParse
    }
}

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
enum Component<'a> {
    Capture(Capture<'a>), // TODO:
    Literal(Cow<'a, str>),
}

impl<'a> Component<'a> {
    pub fn into_owned(self) -> Component<'static> {
        match self {
            Component::Capture(capture) => capture.into_owned().into(),
            Component::Literal(literal) => literal.into_owned().into(),
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
pub struct Pattern<'a> {
    components: Vec<Component<'a>>,
}

impl<'a> Pattern<'a> {
    pub fn into_owned(self) -> Pattern<'static> {
        let Pattern { components } = self;
        let components = components
            .into_iter()
            .map(|component| component.into_owned())
            .collect();
        Pattern { components }
    }
}

impl<'a> Pattern<'a> {
    pub fn parse(text: &'a str) -> Result<Self, TransformError> {
        use nom::bytes::complete as bytes;
        use nom::character::complete as character;
        use nom::error::ParseError;
        use nom::{branch, combinator, multi, sequence, IResult};

        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(bytes::is_not("{"), move |text: &'_ str| {
                Component::from(text)
            })(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            let index = move |text: &'i str| {
                usize::from_str_radix(text, 10)
                    .map(|index| Component::from(Capture::from(index)))
                    .ok()
            };
            let name = move |text: &'i str| Some(Component::from(Capture::from(text)));
            sequence::delimited(
                character::char('{'),
                branch::alt((
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

        fn pattern<'i, E>(input: &'i str) -> IResult<&'i str, Pattern, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map(
                multi::many1(branch::alt((literal, capture))),
                move |components| Pattern { components },
            ))(input)
        }

        pattern::<(_, ErrorKind)>(text)
            .map(|(_, pattern)| pattern)
            .map_err(Into::into)
    }
}

impl FromStr for Pattern<'static> {
    type Err = TransformError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Pattern::parse(text).map(|pattern| pattern.into_owned())
    }
}
