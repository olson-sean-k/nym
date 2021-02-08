use itertools::Itertools;
use std::path::Path;

use crate::glob::{Entry, Glob, GlobError};

// NOTE: If and when raw binary regular expressions are re-introduced,
//       `FromPattern` will no longer be so trivial.

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn read<'a>(
        &'a self,
        directory: impl 'a + AsRef<Path>,
        depth: usize,
    ) -> impl 'a + Iterator<Item = Result<Entry, GlobError>> {
        self.glob.read(directory, depth).filter_map_ok(|entry| {
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
