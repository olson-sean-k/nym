use itertools::Itertools as _;
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
        fs::copy(exactly_one_source(&route)?, route.destination()).map(|_| ())
    }
}

pub enum HardLink {}

impl Operation for HardLink {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        fs::hard_link(exactly_one_source(&route)?, route.destination())
    }
}

pub enum SoftLink {}

#[cfg(unix)]
impl Operation for SoftLink {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        use std::os::unix;

        unix::fs::symlink(exactly_one_source(&route)?, route.destination())
    }
}

#[cfg(windows)]
impl Operation for SoftLink {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        use std::os::windows;

        windows::fs::symlink_file(exactly_one_source(&route)?, route.destination())
    }
}

pub enum Move {}

impl Operation for Move {
    type Routing = Bijective;

    fn write<P>(route: Route<Self::Routing, P>) -> io::Result<()>
    where
        P: AsRef<Path>,
    {
        fs::rename(exactly_one_source(&route)?, route.destination()).map(|_| ())
    }
}

fn exactly_one_source<R, P>(route: &Route<R, P>) -> io::Result<&P>
where
    R: Routing,
    P: AsRef<Path>,
{
    route
        .sources()
        .exactly_one()
        .map_err(|_| Error::new(ErrorKind::Other, "no source paths"))
}
