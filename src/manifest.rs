use bimap::BiMap;
use smallvec::{smallvec, SmallVec};
use std::io::{self, Error, ErrorKind};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

type SourceGroup<P> = SmallVec<[P; 1]>;

pub struct Route<M, P>
where
    P: AsRef<Path>,
{
    sources: SourceGroup<P>,
    destination: P,
    phantom: PhantomData<M>,
}

impl<M, P> Route<M, P>
where
    P: AsRef<Path>,
{
    pub fn sources(&self) -> impl ExactSizeIterator<Item = &'_ P> {
        self.sources.iter()
    }

    pub fn destination(&self) -> &P {
        &self.destination
    }
}

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

    pub fn routes(&self) -> impl ExactSizeIterator<Item = Route<M, &'_ Path>> {
        self.routing.paths().map(|(sources, destination)| Route {
            sources,
            destination,
            phantom: PhantomData,
        })
    }

    pub fn count(&self) -> usize {
        self.routes().len()
    }
}

pub trait Routing: Default {
    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> io::Result<()>;

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SourceGroup<&'_ Path>, &'_ Path)>>;
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

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SourceGroup<&'_ Path>, &'_ Path)>> {
        Box::new(
            self.inner
                .iter()
                .map(|(source, destination)| (smallvec![source.as_ref()], destination.as_ref())),
        )
    }
}
