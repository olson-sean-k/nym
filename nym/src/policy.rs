use faccess::PathExt as _;
use miette::Diagnostic;
use std::path::{Path, PathBuf};
use thiserror::Error;

// TODO: Consider collapsing this module into the crate root and performing
//       per-module checks against `Policy` (i.e., move `PolicyError` and
//       `check` back into the `transform` module).

#[derive(Clone, Debug, Diagnostic, Error)]
#[non_exhaustive]
pub enum PolicyError {
    #[diagnostic(code(nym::policy::destination_not_a_file))]
    #[error("destination is a directory: `{0}`")]
    DestinationNotAFile(PathBuf),
    #[diagnostic(code(nym::policy::destination_already_exists))]
    #[error("destination file already exists: `{0}`")]
    DestinationAlreadyExists(PathBuf),
    #[diagnostic(code(nym::policy::destination_orphaned))]
    #[error("destination parent directory does not exist: `{0}`")]
    DestinationOrphaned(PathBuf),
    #[diagnostic(code(nym::policy::destination_not_writable))]
    #[error("cannot write to destination: `{0}`")]
    DestinationNotWritable(PathBuf),
    #[diagnostic(code(nym::policy::source_not_readable))]
    #[error("cannot read from source: `{0}`")]
    SourceNotReadable(PathBuf),
}

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub parents: bool,
    pub overwrite: bool,
}

// TODO: Are write permissions checked properly here? Parent directories are not
//       queried directly.
pub fn check(
    policy: &Policy,
    source: impl AsRef<Path>,
    destination: impl AsRef<Path>,
) -> Result<(), PolicyError> {
    let source = source.as_ref();
    let destination = destination.as_ref();
    if !source.readable() {
        return Err(PolicyError::SourceNotReadable(source.into()));
    }
    if let Ok(metadata) = destination.metadata() {
        if policy.overwrite {
            if metadata.is_dir() {
                return Err(PolicyError::DestinationNotAFile(destination.into()));
            }
            else if !destination.writable() {
                return Err(PolicyError::DestinationNotWritable(destination.into()));
            }
        }
        else {
            return Err(PolicyError::DestinationAlreadyExists(destination.into()));
        }
    }
    else {
        let parent = destination
            .parent()
            .expect("destination path has no parent");
        if policy.parents {
            let parent = parent
                .ancestors()
                .find(|path| path.exists())
                .expect("destination path has no existing ancestor");
            if !parent.writable() {
                return Err(PolicyError::DestinationNotWritable(destination.into()));
            }
        }
        else {
            if !parent.exists() {
                return Err(PolicyError::DestinationOrphaned(destination.into()));
            }
            if !parent.writable() {
                return Err(PolicyError::DestinationNotWritable(destination.into()));
            }
        }
    }
    Ok(())
}
