use itertools::Itertools;
use std::path::{Path, PathBuf};

use crate::glob::{Entry, Glob, GlobError};

// NOTE: If and when additional from-patterns are supported (such as raw binary
//       regular expressions), `FromPattern` will no longer be so trivial.
//       Moreover, glob types like `Entry` and `Captures` will need to be
//       abstracted away (and `Selector` can be re-introduced).

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    prefix: PathBuf,
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn read<'a>(
        &'a self,
        directory: impl 'a + AsRef<Path>,
        depth: usize,
    ) -> impl 'a + Iterator<Item = Result<Entry, GlobError>> {
        self.glob
            .read(directory.as_ref().join(&self.prefix), depth)
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

impl<'t> From<(PathBuf, Glob<'t>)> for FromPattern<'t> {
    fn from((prefix, glob): (PathBuf, Glob<'t>)) -> Self {
        FromPattern { prefix, glob }
    }
}
