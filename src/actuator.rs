use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Routing};

#[derive(Default)]
pub struct Actuator {
    pub parents: bool,
    pub overwrite: bool,
}

impl Actuator {
    pub fn write<A, I, P>(&self, sources: I, destination: P) -> io::Result<()>
    where
        A: Operation,
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        // TODO: Examine paths to abort overwrites and create directories when
        //       appropriate.
        A::write(sources, destination)
    }
}

pub trait Operation {
    type Routing: Routing;

    fn write<P>(
        sources: impl IntoIterator<Item = P>,
        destination: impl AsRef<Path>,
    ) -> io::Result<()>
    where
        P: AsRef<Path>;
}

pub enum Copy {}

impl Operation for Copy {
    type Routing = Bijective;

    fn write<P>(
        sources: impl IntoIterator<Item = P>,
        destination: impl AsRef<Path>,
    ) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        let source = sources
            .into_iter()
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Other, "no source paths"))?;
        fs::copy(source, destination).map(|_| ())
    }
}

pub enum Move {}

impl Operation for Move {
    type Routing = Bijective;

    fn write<P>(
        sources: impl IntoIterator<Item = P>,
        destination: impl AsRef<Path>,
    ) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        let source = sources
            .into_iter()
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Other, "no source paths"))?;
        fs::rename(source, destination).map(|_| ())
    }
}
