use bimap::BiMap;
use smallvec::{smallvec, SmallVec};
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Manifest<M>
where
    M: Routing,
{
    routing: M,
}

impl<M> Manifest<M>
where
    M: Routing,
{
    pub fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()> {
        self.routing.insert(source.into(), destination.into())
    }

    pub fn paths(&self) -> impl '_ + ExactSizeIterator<Item = (SmallVec<[&'_ Path; 1]>, &'_ Path)> {
        self.routing.paths()
    }

    pub fn count(&self) -> usize {
        self.paths().len()
    }
}

pub trait Routing: Default {
    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> io::Result<()>;

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SmallVec<[&'_ Path; 1]>, &'_ Path)>>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<PathBuf, PathBuf>,
}

impl Routing for Bijective {
    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> io::Result<()> {
        self.inner
            .insert_no_overwrite(source, destination)
            .map_err(|_| Error::new(ErrorKind::Other, "collision"))
    }

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SmallVec<[&'_ Path; 1]>, &'_ Path)>> {
        Box::new(
            self.inner
                .iter()
                .map(|(source, destination)| (smallvec![source.as_ref()], destination.as_ref())),
        )
    }
}
