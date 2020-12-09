mod from;
mod to;

use nom::error::ErrorKind;
use thiserror::Error;

pub use crate::pattern::from::{Find, FromPattern};
pub use crate::pattern::to::ToPattern;

#[derive(Clone, Debug, Error)]
pub enum PatternError {
    #[error("capture not found in from-pattern")]
    CaptureNotFound,
    #[error("failed to parse pattern")]
    Parse,
}

impl<I> From<nom::Err<(I, ErrorKind)>> for PatternError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        PatternError::Parse
    }
}
