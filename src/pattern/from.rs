use itertools::Itertools;
use std::path::{Path, PathBuf};

use crate::glob::{Captures, Glob, GlobError};

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn read(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> impl '_ + Iterator<Item = Result<(PathBuf, Captures<'static>), GlobError>> {
        self.glob
            .clone()
            .read(directory, depth)
            .filter_map_ok(|(entry, captures)| {
                if entry.file_type().is_file() {
                    Some((entry.into_path(), captures))
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
