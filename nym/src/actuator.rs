use itertools::Itertools as _;
use std::fmt::Debug;
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Endpoint, Manifest, Route, Router};
use crate::policy::Policy;

#[derive(Debug)]
pub struct Actuation<W>
where
    W: Operation,
{
    policy: Policy,
    manifest: Manifest<W>,
}

impl<W> Actuation<W>
where
    W: Operation,
{
    pub(crate) fn new(policy: Policy, manifest: Manifest<W>) -> Self {
        Actuation { policy, manifest }
    }

    pub fn write(self) -> io::Result<Manifest<W>> {
        self.write_with(|_| Ok::<_, Error>(()))
    }

    // TODO: Return the manifest when successful and a checkpoint on failure.
    //       To accomplish this, a more general error type will be needed that
    //       can wrap I/O errors.
    pub fn write_with<E, F>(self, mut f: F) -> io::Result<Manifest<W>>
    where
        Error: From<E>,
        F: FnMut(&Route<W::Router>) -> Result<(), E>,
    {
        let Actuation { policy, manifest } = self;
        for route in manifest.routes() {
            if policy.parents {
                for path in route.destination().paths() {
                    if let Some(parent) = path.parent().filter(|parent| !parent.exists()) {
                        fs::create_dir_all(parent)?;
                    }
                }
            }
            f(&route)?;
            W::write(route)?;
        }
        Ok(manifest)
    }

    pub fn manifest(&self) -> &Manifest<W> {
        &self.manifest
    }
}

pub trait Operation: 'static {
    type Router: Debug + Router;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()>;
}

// TODO: How useful is appending? Perhaps this need not be supported at all.
pub enum Append {}

pub enum Copy {}

impl Operation for Copy {
    type Router = Bijective;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()> {
        fs::copy(
            exactly_one_path(route.source())?,
            exactly_one_path(route.destination())?,
        )
        .map(|_| ())
    }
}

pub enum HardLink {}

impl Operation for HardLink {
    type Router = Bijective;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()> {
        fs::hard_link(
            exactly_one_path(route.source())?,
            exactly_one_path(route.destination())?,
        )
    }
}

pub enum SoftLink {}

#[cfg(unix)]
impl Operation for SoftLink {
    type Router = Bijective;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()> {
        use std::os::unix;

        unix::fs::symlink(
            exactly_one_path(route.source())?,
            exactly_one_path(route.destination())?,
        )
    }
}

#[cfg(windows)]
impl Operation for SoftLink {
    type Router = Bijective;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()> {
        use std::os::windows;

        windows::fs::symlink_file(
            exactly_one_path(route.source())?,
            exactly_one_path(route.destination())?,
        )
    }
}

pub enum Move {}

impl Operation for Move {
    type Router = Bijective;

    fn write(route: Route<'_, Self::Router>) -> io::Result<()> {
        fs::rename(
            exactly_one_path(route.source())?,
            exactly_one_path(route.destination())?,
        )
        .map(|_| ())
    }
}

fn exactly_one_path(endpoint: &impl Endpoint) -> io::Result<&Path> {
    endpoint
        .paths()
        .exactly_one()
        .map_err(|_| Error::new(ErrorKind::Other, "unexpected number of endpoint paths"))
}
