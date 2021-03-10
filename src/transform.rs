use faccess::PathExt as _;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::environment::Environment;
use crate::manifest::{Manifest, ManifestError, Routing};
use crate::pattern::{FromPattern, PatternError, ToPattern};

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TransformError {
    #[error("failed to traverse directory tree: {0}")]
    Read(PatternError),
    #[error("failed to resolve to-pattern: {0}")]
    PatternResolution(PatternError),
    #[error("failed to insert route: {0}")]
    RouteInsertion(ManifestError),
    #[error("destination is a directory: `{0}`")]
    DestinationNotAFile(PathBuf),
    #[error("destination file already exists: `{0}`")]
    DestinationAlreadyExists(PathBuf),
    #[error("destination parent directory does not exist: `{0}`")]
    DestinationOrphaned(PathBuf),
    #[error("cannot write to destination: `{0}`")]
    DestinationNotWritable(PathBuf),
    #[error("cannot read from source: `{0}`")]
    SourceNotReadable(PathBuf),
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
        #[cfg(windows)]
        fn normalize(path: impl Into<PathBuf>) -> PathBuf {
            use path_slash::PathBufExt as _;

            PathBuf::from_slash_lossy(path.into())
        }

        #[cfg(not(windows))]
        #[inline(always)]
        fn normalize(path: impl Into<PathBuf>) -> PathBuf {
            path.into()
        }

        let mut manifest = Manifest::default();
        for entry in self.from.read(directory.as_ref(), depth) {
            let entry = entry.map_err(TransformError::Read)?;
            let source = entry.path();
            let mut destination = directory.as_ref().to_path_buf();
            destination.push(
                self.to
                    .resolve(&source, entry.captures())
                    .map_err(TransformError::PatternResolution)?,
            );
            self.verify_route_policy(source, &destination)?;
            manifest
                .insert(normalize(source), normalize(destination))
                .map_err(TransformError::RouteInsertion)?;
        }
        Ok(manifest)
    }

    // TODO: Are write permissions checked properly here? Parent directories are
    //       not queried directly.
    fn verify_route_policy(
        &self,
        source: impl AsRef<Path>,
        destination: impl AsRef<Path>,
    ) -> Result<(), TransformError> {
        let policy = self.environment.policy();
        let source = source.as_ref();
        let destination = destination.as_ref();
        if !source.readable() {
            return Err(TransformError::SourceNotReadable(source.into()));
        }
        if let Ok(metadata) = destination.metadata() {
            if policy.overwrite {
                if metadata.is_dir() {
                    return Err(TransformError::DestinationNotAFile(destination.into()));
                }
                else if !destination.writable() {
                    return Err(TransformError::DestinationNotWritable(destination.into()));
                }
            }
            else {
                return Err(TransformError::DestinationAlreadyExists(destination.into()));
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
                    return Err(TransformError::DestinationNotWritable(destination.into()));
                }
            }
            else {
                if !parent.exists() {
                    return Err(TransformError::DestinationOrphaned(destination.into()));
                }
                if !parent.writable() {
                    return Err(TransformError::DestinationNotWritable(destination.into()));
                }
            }
        }
        Ok(())
    }
}
