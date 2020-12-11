use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Manifest};

// DANGER: Use at your own risk! Writing to the file system may cause
//         unrecoverable data loss!

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
