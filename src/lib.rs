pub mod copy;
pub mod edit;
pub mod r#move;

use nom::branch;
use nom::bytes::complete as bytes;
use nom::character::complete as character;
use nom::combinator;
use nom::error::{ErrorKind, ParseError};
use nom::multi;
use nom::sequence;
use nom::{self, IResult};
use std::io;
use std::str::FromStr;

#[derive(Clone, Debug)]
enum Component {
    Literal(String),
    Capture(usize),
}

#[derive(Clone, Debug)]
pub struct Pattern {
    components: Vec<Component>,
}

impl FromStr for Pattern {
    // TODO: Use a bona fide error type.
    type Err = io::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(
                combinator::verify(
                    bytes::take_while(
                        move |x| x != '#',
                    ),
                    move |text: &'_ str| !text.is_empty(),
                ),
                move |text: &'_ str| Component::Literal(text.to_string()),
            )(input)
        }

        fn capture<'i, E>(input: &'i str) -> IResult<&'i str, Component, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map_res(
                sequence::preceded(
                    character::char('#'),
                    sequence::delimited(
                        character::char('['),
                        character::digit1,
                        character::char(']'),
                    ),
                ),
                move |index| usize::from_str_radix(index, 10).map(|index| Component::Capture(index)),
            )(input)
        }

        fn parse<'i, E>(input: &'i str) -> IResult<&'i str, Pattern, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(
                multi::many1(
                    branch::alt((
                        literal,
                        capture,
                    )),
                ),
                move |components| Pattern { components },
            )(input)
        }

        // TODO: Do not unwrap.
        Ok(parse::<(_, ErrorKind)>(text).map(|(_, pattern)| pattern).expect("PATTERN"))
    }
}
