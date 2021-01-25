use faccess::PathExt as _;
use std::path::{Path, PathBuf};
use thiserror::Error;
use walkdir::WalkDir;

use crate::environment::Environment;
use crate::manifest::{Manifest, ManifestError, Routing};
use crate::pattern::{Candidate, FromPattern, PatternError, ToPattern};

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("failed to traverse directory tree: {0}")]
    ReadTree(walkdir::Error),
    #[error("failed to resolve to-pattern: {0}")]
    PatternResolution(PatternError),
    #[error("failed to insert route: {0}")]
    Route(ManifestError),
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
        let mut manifest = Manifest::default();
        // TODO: This is inefficient, since glob from-patterns determine if
        //       recursion is necessary or should continue when a particular
        //       sub-tree is reached. Consider emitting an iterator that
        //       traverses the directory tree from `FromPattern`.
        for entry in WalkDir::new(directory.as_ref())
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth)
        {
            let entry = entry.map_err(|error| TransformError::ReadTree(error))?;
            if entry.file_type().is_file() {
                let source = entry.path();
                let candidate = Candidate::tree(directory.as_ref(), source);
                if let Some(captures) = self.from.captures(&candidate) {
                    let mut destination = candidate.destination().to_path_buf();
                    destination.push(
                        self.to
                            .resolve(source, &captures)
                            .map_err(|error| TransformError::PatternResolution(error))?,
                    );
                    let (source, destination) = self.try_apply_policy(source, destination)?;
                    let source = normalize(source);
                    let destination = normalize(destination);
                    manifest
                        .insert(source, destination)
                        .map_err(|error| TransformError::Route(error))?;
                }
            }
        }
        Ok(manifest)
    }

    // TODO: Are write permissions checked properly here? Parent directories are
    //       not queried directly.
    fn try_apply_policy(
        &self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> Result<(PathBuf, PathBuf), TransformError> {
        let policy = self.environment.policy();
        let source = source.into();
        let destination = destination.into();
        if !source.readable() {
            return Err(TransformError::SourceNotReadable(source));
        }
        if let Ok(metadata) = destination.metadata() {
            if policy.overwrite {
                if metadata.is_dir() {
                    return Err(TransformError::DestinationNotAFile(destination));
                }
                else if !destination.writable() {
                    return Err(TransformError::DestinationNotWritable(destination));
                }
            }
            else {
                return Err(TransformError::DestinationAlreadyExists(destination));
            }
        }
        else {
            let parent = destination
                .parent()
                .expect("destination path has no parent");
            if policy.parents {
                let parent = parent
                    .ancestors()
                    .filter(|path| path.exists())
                    .next()
                    .expect("destination path has no existing ancestor");
                if !parent.writable() {
                    return Err(TransformError::DestinationNotWritable(destination));
                }
            }
            else {
                if !parent.exists() {
                    return Err(TransformError::DestinationOrphaned(destination));
                }
                if !parent.writable() {
                    return Err(TransformError::DestinationNotWritable(destination));
                }
            }
        }
        Ok((source, destination))
    }
}

#[cfg(windows)]
fn normalize(path: PathBuf) -> PathBuf {
    use path_slash::PathBufExt as _;

    PathBuf::from_slash_lossy(path)
}

#[cfg(not(windows))]
#[inline(always)]
fn normalize(path: PathBuf) -> PathBuf {
    path
}
