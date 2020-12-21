use bimap::BiMap;
use smallvec::{smallvec, SmallVec};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use thiserror::Error;

type SourceGroup<P> = SmallVec<[P; 1]>;

#[derive(Debug, Error)]
pub enum ManifestError {
    #[error("detected collision in route destination path: {0}")]
    PathCollision(PathBuf),
}

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
    M: Router,
{
    router: M,
}

impl<M> Manifest<M>
where
    M: Router,
{
    pub fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> Result<(), ManifestError> {
        self.router.insert(source.into(), destination.into())
    }

    pub fn routes(&self) -> impl ExactSizeIterator<Item = Route<M, &'_ Path>> {
        self.router.paths().map(|(sources, destination)| Route {
            sources,
            destination,
            phantom: PhantomData,
        })
    }
}

pub trait Router: Default {
    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> Result<(), ManifestError>;

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SourceGroup<&'_ Path>, &'_ Path)>>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<PathBuf, PathBuf>,
}

impl Router for Bijective {
    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> Result<(), ManifestError> {
        if self.inner.contains_right(&destination) {
            Err(ManifestError::PathCollision(destination))
        }
        else {
            Ok(self.inner.insert_no_overwrite(source, destination).unwrap())
        }
    }

    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = (SourceGroup<&'_ Path>, &'_ Path)>> {
        Box::new(
            self.inner
                .iter()
                .map(|(source, destination)| (smallvec![source.as_ref()], destination.as_ref())),
        )
    }
}
