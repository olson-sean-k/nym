use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Route, Routing};

#[derive(Default)]
pub struct Actuator {
    pub parents: bool,
    pub overwrite: bool,
}

impl Actuator {
    pub fn write<A, P>(&self, route: Route<A::Routing, P>) -> io::Result<()>
    where
        A: Operation,
        P: AsRef<Path>,
    {
        // TODO: Examine paths to abort overwrites and create directories when
        //       appropriate.
        A::write(route)
    }
}

pub trait Operation {
    type Routing: Routing;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>;
}

pub enum Copy {}

impl Operation for Copy {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        let source = route
            .sources()
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Other, "no source paths"))?;
        fs::copy(source, route.destination()).map(|_| ())
    }
}

pub enum Move {}

impl Operation for Move {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        let source = route
            .sources()
            .next()
            .ok_or_else(|| Error::new(ErrorKind::Other, "no source paths"))?;
        fs::rename(source, route.destination()).map(|_| ())
    }
}
