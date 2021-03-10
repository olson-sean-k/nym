use itertools::Itertools;
use std::path::Path;

use crate::glob::{Entry, Glob};
use crate::pattern::PatternError;

#[derive(Clone, Debug)]
pub struct FromPattern<'t> {
    glob: Glob<'t>,
}

impl<'t> FromPattern<'t> {
    pub fn read<'a>(
        &'a self,
        directory: impl 'a + AsRef<Path>,
        depth: usize,
    ) -> impl 'a + Iterator<Item = Result<Entry, PatternError>> {
        self.glob
            .read(directory, depth)
            .map(|entry| entry.map_err(From::from))
            .filter_map_ok(|entry| entry.file_type().is_file().then(|| entry))
    }
}

impl<'t> From<Glob<'t>> for FromPattern<'t> {
    fn from(glob: Glob<'t>) -> Self {
        FromPattern { glob }
    }
}
