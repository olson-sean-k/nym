use bimap::BiMap;
use miette::Diagnostic;
use smallvec::{Array, SmallVec};
use std::fmt::{self, Debug, Formatter};
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::actuator::{Copy, HardLink, Move, Operation, SoftLink};

#[derive(Debug, Diagnostic, Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[diagnostic(code(nym::manifest::collision))]
    #[error("detected collision in route destination path: `{0}`")]
    PathCollision(PathBuf),
}

pub trait Endpoint {
    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = &'_ Path>>;
}

impl Endpoint for PathBuf {
    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = &'_ Path>> {
        Box::new([self.as_ref()].into_iter())
    }
}

impl<const N: usize> Endpoint for SmallVec<[PathBuf; N]>
where
    [PathBuf; N]: Array<Item = PathBuf>,
{
    fn paths(&self) -> Box<dyn '_ + ExactSizeIterator<Item = &'_ Path>> {
        Box::new(self.iter().map(|path| path.as_ref()))
    }
}

pub trait Router: Clone + Default {
    type Source: Endpoint;
    type Destination: Endpoint;

    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> Result<(), ManifestError>;

    fn routes(&self) -> Box<dyn '_ + ExactSizeIterator<Item = Route<'_, Self>>>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    bimap: BiMap<PathBuf, PathBuf>,
}

impl Router for Bijective {
    type Source = PathBuf;
    type Destination = PathBuf;

    fn insert(&mut self, source: PathBuf, destination: PathBuf) -> Result<(), ManifestError> {
        self.bimap
            .insert_no_overwrite(source, destination)
            .map_err(|(_, destination)| ManifestError::PathCollision(destination))
    }

    fn routes(&self) -> Box<dyn '_ + ExactSizeIterator<Item = Route<'_, Self>>> {
        Box::new(self.bimap.iter().map(|(source, destination)| Route {
            source,
            destination,
        }))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Route<'e, R>
where
    R: Router,
{
    source: &'e R::Source,
    destination: &'e R::Destination,
}

impl<'e, R> Route<'e, R>
where
    R: Router,
{
    pub fn source(&self) -> &R::Source {
        self.source
    }

    pub fn destination(&self) -> &R::Destination {
        self.destination
    }
}

#[derive(Clone)]
pub enum ManifestEnvelope {
    //Append(Manifest<Append>),
    Copy(Manifest<Copy>),
    HardLink(Manifest<HardLink>),
    Move(Manifest<Move>),
    SoftLink(Manifest<SoftLink>),
}

pub struct Manifest<W>
where
    W: Operation,
{
    router: W::Router,
}

impl<W> Manifest<W>
where
    W: Operation,
{
    pub fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> Result<(), ManifestError> {
        self.router.insert(source.into(), destination.into())
    }

    pub fn routes(&self) -> impl ExactSizeIterator<Item = Route<'_, W::Router>> {
        self.router.routes()
    }
}

impl<W> Clone for Manifest<W>
where
    W: Operation,
{
    fn clone(&self) -> Self {
        Manifest {
            router: self.router.clone(),
        }
    }
}

impl<W> Debug for Manifest<W>
where
    W: Operation,
    W::Router: Debug,
{
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Manifest")
            .field("router", &self.router)
            .finish()
    }
}

impl<W> Default for Manifest<W>
where
    W: Operation,
{
    fn default() -> Self {
        Manifest {
            router: Default::default(),
        }
    }
}
