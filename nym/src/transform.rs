use faccess::PathExt as _;
use miette::Diagnostic;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::environment::Environment;
use crate::manifest::{Manifest, ManifestError, Routing};
use crate::pattern::{FromPattern, FromPatternError, ToPattern, ToPatternError};

#[derive(Debug, Diagnostic, Error)]
#[diagnostic(transparent)]
#[error("failed to build manifest")]
pub struct TransformError {
    #[source]
    kind: ErrorKind,
}

impl From<FromPatternError> for TransformError {
    fn from(error: FromPatternError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<ManifestError> for TransformError {
    fn from(error: ManifestError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<PolicyError> for TransformError {
    fn from(error: PolicyError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<ToPatternError> for TransformError {
    fn from(error: ToPatternError) -> Self {
        TransformError { kind: error.into() }
    }
}

#[derive(Debug, Diagnostic, Error)]
enum PolicyError {
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

#[derive(Debug, Diagnostic, Error)]
enum ErrorKind {
    #[diagnostic(transparent)]
    #[error(transparent)]
    FromPattern(#[from] FromPatternError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    ToPattern(#[from] ToPatternError),
}

#[derive(Clone, Debug)]
pub struct Transform<'e, 'f, 't> {
    environment: &'e Environment,
    from: FromPattern<'f>,
    to: ToPattern<'t>,
}

impl<'e, 'f, 't> Transform<'e, 'f, 't> {
    pub(in crate) fn new(
        environment: &'e Environment,
        from: FromPattern<'f>,
        to: ToPattern<'t>,
    ) -> Self {
        Transform {
            environment,
            from,
            to,
        }
    }

    pub fn read<M>(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Manifest<M>, TransformError>
    where
        M: Routing,
    {
        let mut manifest = Manifest::default();
        for entry in self.from.walk(directory.as_ref(), depth) {
            let entry = entry?;
            let source = entry.path();
            let mut destination = directory.as_ref().to_path_buf();
            destination.push(self.to.resolve(&source, entry.matched())?);
            self.verify_route_policy(source, &destination)?;
            manifest.insert(source, destination)?;
        }
        Ok(manifest)
    }

    // TODO: Are write permissions checked properly here? Parent directories are
    //       not queried directly.
    fn verify_route_policy(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), PolicyError> {
        let policy = self.environment.policy();
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
}
