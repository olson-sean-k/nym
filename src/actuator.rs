#[allow(unused_imports)]
use std::fs;
use std::io::{self, Error, ErrorKind};
use std::path::Path;

use crate::manifest::{Bijective, Manifest};

pub trait Actuator {
    type Manifest: Manifest;

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
        // DANGER: Use at your own risk! Writing to the file system may cause
        //         unrecoverable data loss!
        //fs::copy(source, destination)
        //println!("copy {:?} -> {:?}", source.as_ref(), destination.as_ref());
        Ok(())
    }
}

pub enum Move {}

impl Actuator for Move {
    type Manifest = Bijective;

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
        // DANGER: Use at your own risk! Writing to the file system may cause
        //         unrecoverable data loss!
        //fs::rename(source, destination)
        //println!("move {:?} -> {:?}", source.as_ref(), destination.as_ref());
        Ok(())
    }
}
