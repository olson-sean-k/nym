mod from;
mod to;

use nom::error::ErrorKind;
use std::io;
use std::str::Utf8Error;
use thiserror::Error;

pub use crate::pattern::from::FromPattern;
pub use crate::pattern::to::ToPattern;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("capture not found in from-pattern")]
    CaptureNotFound,
    #[error("failed to parse pattern")]
    Parse,
    #[error("failed to encode capture in to-pattern: {0}")]
    Encoding(Utf8Error),
    #[error("failed to read property in to-pattern: {0}")]
    ReadProperty(io::Error),
}

impl<I> From<nom::Err<(I, ErrorKind)>> for PatternError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        PatternError::Parse
    }
}
