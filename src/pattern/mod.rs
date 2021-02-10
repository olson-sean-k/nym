mod from;
mod to;

use nom::error::ErrorKind;
use std::io;
use std::str::Utf8Error;
use thiserror::Error;

pub use crate::pattern::from::FromPattern;
pub use crate::pattern::to::ToPattern;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PatternError {
    #[error("capture not found in from-pattern")]
    CaptureNotFound,
    #[error("failed to parse pattern: {0}")]
    Parse(nom::Err<(String, ErrorKind)>),
    #[error("failed to encode capture in to-pattern: {0}")]
    Encoding(Utf8Error),
    #[error("failed to read property in to-pattern: {0}")]
    Property(io::Error),
}

impl<'i> From<nom::Err<(&'i str, ErrorKind)>> for PatternError {
    fn from(error: nom::Err<(&'i str, ErrorKind)>) -> Self {
        PatternError::Parse(error.to_owned())
    }
}
