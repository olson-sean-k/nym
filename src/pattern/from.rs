use itertools::Itertools;
use std::path::Path;

use crate::glob::{Entry, Glob, GlobError};

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn read(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> impl '_ + Iterator<Item = Result<Entry, GlobError>> {
        self.glob
            .clone()
            .read(directory, depth)
            .filter_map_ok(|entry| {
                if entry.file_type().is_file() {
                    Some(entry)
                }
                else {
                    None
                }
            })
    }
}

impl<'t> From<Glob<'t>> for FromPattern<'t> {
    fn from(glob: Glob<'t>) -> Self {
        FromPattern { glob }
    }
}
