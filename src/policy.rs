use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("destination is a directory: {0}")]
    NotAFile(PathBuf),
    #[error("destination file already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error("destination parent directory does not exist: {0}")]
    Orphaned(PathBuf),
}

pub struct Policy {
    pub parents: bool,
    pub overwrite: bool,
}

impl Policy {
    pub fn read(&self, destination: impl AsRef<Path>) -> Result<(), PolicyError> {
        let destination = destination.as_ref();
        let parent = destination
            .parent()
            .expect("destination path has no parent");
        if !self.parents && !parent.exists() {
            return Err(PolicyError::Orphaned(destination.to_path_buf()));
        }
        if let Ok(metadata) = destination.metadata() {
            if metadata.is_dir() {
                return Err(PolicyError::NotAFile(destination.to_path_buf()));
            }
            else if !self.overwrite {
                return Err(PolicyError::AlreadyExists(destination.to_path_buf()));
            }
        }
        Ok(())
    }

    pub fn write(&self, destination: impl AsRef<Path>) -> io::Result<()> {
        if self.parents {
            let parent = destination
                .as_ref()
                .parent()
                .expect("destination path has no parent");
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        Ok(())
    }
}
