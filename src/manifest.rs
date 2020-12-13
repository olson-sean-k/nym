use bimap::BiMap;
use std::io::{self, Error, ErrorKind};
use std::path::PathBuf;

pub trait Manifest: Default {
    type SourceGroup: Clone + IntoIterator<Item = PathBuf>;

    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()>;

    // TODO: This is a compromise. Without GATs and `impl Trait`, it is very
    //       difficult to expose iterators over references for reads. Instead,
    //       manifests must be convertible into a common type that supports
    //       many-to-one relationships between paths.
    fn into_grouped_paths(self) -> Vec<(Self::SourceGroup, PathBuf)>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<PathBuf, PathBuf>,
}

impl IntoIterator for Bijective {
    type Item = <BiMap<PathBuf, PathBuf> as IntoIterator>::Item;
    type IntoIter = <BiMap<PathBuf, PathBuf> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl Manifest for Bijective {
    type SourceGroup = Option<PathBuf>;

    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()> {
        self.inner
            .insert_no_overwrite(source.into(), destination.into())
            .map_err(|_| Error::new(ErrorKind::Other, "bijective collision"))
    }

    fn into_grouped_paths(self) -> Vec<(Self::SourceGroup, PathBuf)> {
        self.inner
            .into_iter()
            .map(|(source, terminal)| (Some(source), terminal))
            .collect()
    }
}
