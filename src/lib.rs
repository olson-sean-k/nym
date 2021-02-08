#![doc(
    html_logo_url = "https://raw.githubusercontent.com/olson-sean-k/nym/master/doc/nym.svg?sanitize=true"
)]

pub mod actuator;
pub mod environment;
pub mod glob;
pub mod manifest;
pub mod memoize;
pub mod pattern;
pub mod transform;

use itertools::Position;
use std::borrow::Borrow;

trait PositionExt<T> {
    fn lift(&self) -> (Position<()>, &T);

    fn interior_borrow<B>(&self) -> Position<&B>
    where
        T: Borrow<B>;
}

impl<T> PositionExt<T> for Position<T> {
    fn lift(&self) -> (Position<()>, &T) {
        match *self {
            Position::First(ref inner) => (Position::First(()), inner),
            Position::Middle(ref inner) => (Position::Middle(()), inner),
            Position::Last(ref inner) => (Position::Last(()), inner),
            Position::Only(ref inner) => (Position::Only(()), inner),
        }
    }

    fn interior_borrow<B>(&self) -> Position<&B>
    where
        T: Borrow<B>,
    {
        match *self {
            Position::First(ref inner) => Position::First(inner.borrow()),
            Position::Middle(ref inner) => Position::Middle(inner.borrow()),
            Position::Last(ref inner) => Position::Last(inner.borrow()),
            Position::Only(ref inner) => Position::Only(inner.borrow()),
        }
    }
}
