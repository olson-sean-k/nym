use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::environment::Environment;
use crate::manifest::{Bijective, Route, Routing};

#[derive(Clone, Debug)]
pub struct Actuator<'e> {
    environment: &'e Environment,
}

impl<'e> Actuator<'e> {
    pub(in crate) fn new(environment: &'e Environment) -> Self {
        Actuator { environment }
    }

    pub fn write<A, P>(&self, route: Route<A::Routing, P>) -> io::Result<()>
    where
        A: Operation,
        P: AsRef<Path>,
    {
        let policy = self.environment.policy();
        if policy.parents {
            let parent = route
                .destination()
                .as_ref()
                .parent()
                .expect("destination path has no parent");
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        A::write(route)
    }
}

pub trait Operation {
    type Routing: Routing;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>;
}

// TODO: How useful is appending? Perhaps this need not be supported at all.
pub enum Append {}

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

pub enum HardLink {}

pub enum SoftLink {}

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
