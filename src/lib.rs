pub mod actuator;
pub mod glob;
pub mod manifest;
pub mod pattern;
pub mod policy;
pub mod transform;

use itertools::Position;

trait PositionExt<T> {
    fn lift(&self) -> (Position<()>, &T);
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
}
