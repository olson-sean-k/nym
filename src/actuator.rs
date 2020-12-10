use bimap::BiMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::transform::Manifest;

pub trait Actuator {
    type Manifest: Manifest;

    fn write(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<()>;
}

pub enum Copy {}

impl Actuator for Copy {
    type Manifest = BiMap<PathBuf, PathBuf>;

    fn write(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<()> {
        // DANGER: Use at your own risk! Writing to the file system may cause
        //         unrecoverable data loss!
        //fs::copy(source, destination)
        println!("copy {:?} -> {:?}", source.as_ref(), destination.as_ref());
        Ok(())
    }
}

pub enum Move {}

impl Actuator for Move {
    type Manifest = BiMap<PathBuf, PathBuf>;

    fn write(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> io::Result<()> {
        // DANGER: Use at your own risk! Writing to the file system may cause
        //         unrecoverable data loss!
        //fs::rename(source, destination)
        println!("move {:?} -> {:?}", source.as_ref(), destination.as_ref());
        Ok(())
    }
}
