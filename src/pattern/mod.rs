mod from;
mod to;

use nom::error::ErrorKind;
use std::io;
use thiserror::Error;

pub use crate::pattern::from::{Find, FromPattern};
pub use crate::pattern::to::ToPattern;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("capture not found in from-pattern")]
    CaptureNotFound,
    #[error("failed to parse pattern")]
    Parse,
    #[error("failed to read property in to-pattern: {0}")]
    ReadProperty(io::Error),
    #[error("failed to read directory tree: {0}")]
    ReadTree(walkdir::Error),
}

impl<I> From<nom::Err<(I, ErrorKind)>> for PatternError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        PatternError::Parse
    }
}
