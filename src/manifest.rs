use bimap::BiMap;
use std::convert::TryInto;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;

use crate::path::CanonicalPath;

pub trait Manifest: Default {
    type SourceGroup: Clone + IntoIterator<Item = CanonicalPath>;

    fn insert(&mut self, source: CanonicalPath, destination: CanonicalPath) -> io::Result<()>;

    // TODO: This is a compromise. Without GATs and `impl Trait`, it is very
    //       difficult to expose iterators over references for reads. Instead,
    //       manifests must be convertible into a common type that supports
    //       many-to-one relationships between paths.
    fn into_grouped_paths(self) -> Vec<(Self::SourceGroup, CanonicalPath)>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<CanonicalPath, CanonicalPath>,
}

impl Manifest for Bijective {
    type SourceGroup = Option<CanonicalPath>;

    fn insert(&mut self, source: CanonicalPath, destination: CanonicalPath) -> io::Result<()> {
        self.inner
            .insert_no_overwrite(source, destination)
            .map_err(|_| Error::new(ErrorKind::Other, "bijective collision"))
    }

    fn into_grouped_paths(self) -> Vec<(Self::SourceGroup, CanonicalPath)> {
        self.inner
            .into_iter()
            .map(|(source, terminal)| (Some(source), terminal))
            .collect()
    }
}
