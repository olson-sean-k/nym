use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Route, Router};
use crate::policy::Policy;

#[derive(Default)]
pub struct Actuator;

impl Actuator {
    pub fn write<A, P>(&self, policy: &Policy, route: Route<A::Router, P>) -> io::Result<()>
    where
        A: Operation,
        P: AsRef<Path>,
    {
        policy.write(route.destination())?;
        A::write(route)
    }
}

pub trait Operation {
    type Router: Router;

    fn write<P>(route: Route<Self::Router, P>) -> io::Result<()>
    where
        P: AsRef<Path>;
}

pub enum Append {}

pub enum Copy {}

impl Operation for Copy {
    type Router = Bijective;

    fn write<P>(route: Route<Self::Router, P>) -> io::Result<()>
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

pub enum Link {}

pub enum Move {}

impl Operation for Move {
    type Router = Bijective;

    fn write<P>(route: Route<Self::Router, P>) -> io::Result<()>
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
