use itertools::Itertools;
use std::path::{Path, PathBuf};
use wax::{Glob, GlobError, WalkEntry};

pub type FromPatternError = GlobError<'static>;

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    prefix: PathBuf,
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn new(text: &'t str) -> Result<Self, FromPatternError> {
        Glob::partitioned(text)
            .map(|(prefix, glob)| FromPattern { prefix, glob })
            .map_err(GlobError::into_owned)
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
