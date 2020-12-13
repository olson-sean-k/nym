use fool::BoolExt as _;
use normpath::PathExt as _;
use smallvec::SmallVec;
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};

use crate::manifest::{Bijective, Manifest};

// DANGER: Use at your own risk! Writing to the file system may cause
//         unrecoverable data loss!

#[derive(Default)]
pub struct Environment {
    root: Option<PathBuf>,
}

impl Environment {
    pub fn with_root(root: impl Into<Option<PathBuf>>) -> io::Result<Self> {
        match root.into() {
            Some(root) => {
                let root = root.normalize()?;
                root.has_root()
                    .then_ext(|| Environment {
                        root: Some(root.into_path_buf()),
                    })
                    .ok_or_else(|| Error::new(ErrorKind::Other, "non-root path"))
            }
            _ => Ok(Environment { root: None }),
        }
    }

    // TODO: `impl Trait` is not used here, because it disallows explicitly
    //       setting the input type parameter `A`.
    pub fn write<A, I, O>(&self, sources: I, destination: O) -> io::Result<()>
    where
        A: Actuator,
        I: IntoIterator,
        I::Item: AsRef<Path>,
        O: AsRef<Path>,
    {
        // TODO: Refactor this so that this check can be performed before
        //       attempting to actuate.
        if let Some(root) = self.root.as_ref() {
            let sources = sources
                .into_iter()
                .map(|path| {
                    path.as_ref()
                        .starts_with(root)
                        .then_ext(|| path)
                        .ok_or_else(|| Error::new(ErrorKind::Other, "path not in root"))
                })
                .collect::<Result<SmallVec<[_; 1]>, _>>()?;
            let destination = destination
                .as_ref()
                .starts_with(root)
                .then_ext(|| destination)
                .ok_or_else(|| Error::new(ErrorKind::Other, "path not in root"))?;
            A::write(sources, destination)
        }
        else {
            A::write(sources, destination)
        }
    }
}

pub trait Actuator {
    type Manifest: Manifest;

    const NAME: &'static str;

    fn write<P>(
        sources: impl IntoIterator<Item = P>,
        destination: impl AsRef<Path>,
    ) -> io::Result<()>
    where
        P: AsRef<Path>;
}

pub enum Copy {}

impl Actuator for Copy {
    type Manifest = Bijective;

    const NAME: &'static str = "copy";

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
            .ok_or_else(|| Error::new(ErrorKind::Other, ""))?;
        fs::copy(source, destination).map(|_| ())
    }
}

pub enum Move {}

impl Actuator for Move {
    type Manifest = Bijective;

    const NAME: &'static str = "move";

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
            .ok_or_else(|| Error::new(ErrorKind::Other, ""))?;
        fs::rename(source, destination).map(|_| ())
    }
}
