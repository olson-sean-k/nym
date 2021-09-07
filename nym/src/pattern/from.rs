use itertools::Itertools;
use miette::Report;
use std::path::{Path, PathBuf};
use thiserror::Error;
use wax::{Glob, GlobError, WalkEntry};

// TODO: This is a temporary stopgap. Do not convert to a `Report` immediately.
//       Refactor errors and propagate diagnostics such that binaries can decide
//       how to use them (if at all).
#[derive(Debug, Error)]
#[error("{0:?}")]
pub struct FromPatternError(Report);

impl<'t> From<GlobError<'t>> for FromPatternError {
    fn from(error: GlobError<'t>) -> Self {
        FromPatternError(Report::from(error.into_owned()))
    }
}

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    prefix: PathBuf,
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn new(text: &'t str) -> Result<Self, FromPatternError> {
        Glob::partitioned(text)
            .map(|(prefix, glob)| FromPattern { prefix, glob })
            .map_err(From::from)
    }

    pub fn walk(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> impl Iterator<Item = Result<WalkEntry, FromPatternError>> {
        self.glob
            .walk(directory.as_ref().join(&self.prefix), depth)
            .filter_map_ok(|entry| {
                if entry.file_type().is_file() {
                    Some(entry)
                }
                else {
                    None
                }
            })
            .map(|result| result.map_err(From::from))
    }

    pub fn has_semantic_literals(&self) -> bool {
        self.glob.has_semantic_literals()
    }
}
