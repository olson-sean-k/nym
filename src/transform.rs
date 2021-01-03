use std::path::Path;
use thiserror::Error;
use walkdir::WalkDir;

use crate::glob::BytePath;
use crate::manifest::{Manifest, ManifestError, Router};
use crate::pattern::{FromPattern, PatternError, ToPattern};
use crate::policy::{Policy, PolicyError};

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("failed to traverse directory tree: {0}")]
    DirectoryTraversal(walkdir::Error),
    #[error("invalid destination: {0}")]
    DestinationInvalid(PolicyError),
    #[error("failed to resolve to-pattern: {0}")]
    PatternResolution(PatternError),
    #[error("invalid manifest: {0}")]
    ManifestInvalid(ManifestError),
}

impl From<ManifestError> for TransformError {
    fn from(error: ManifestError) -> Self {
        TransformError::ManifestInvalid(error)
    }
}

impl From<PolicyError> for TransformError {
    fn from(error: PolicyError) -> Self {
        TransformError::DestinationInvalid(error)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MatchStrategy {
    File,
    Path,
}

impl MatchStrategy {
    fn subpath<'p>(&self, path: &'p Path) -> Option<BytePath<'p>> {
        match *self {
            MatchStrategy::File => path.file_name().map(|name| BytePath::from_os_str(name)),
            MatchStrategy::Path => Some(BytePath::from_path(path)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Transform<'f, 't> {
    pub from: FromPattern<'f>,
    pub to: ToPattern<'t>,
    pub strategy: MatchStrategy,
}

impl<'t, 'f> Transform<'t, 'f> {
    pub fn read<M>(
        &self,
        policy: &Policy,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Manifest<M>, TransformError>
    where
        M: Router,
    {
        // TODO: `FromPattern` should control iteration. Revisit emitting an
        //       iterator from `FromPattern` instead. Importantly, globs may or
        //       may not require traversing subdirectories, while regular
        //       expressions cannot specify traversals intrinsically.
        let mut manifest = Manifest::default();
        for entry in WalkDir::new(directory.as_ref())
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth)
        {
            let entry = entry.map_err(|error| TransformError::DirectoryTraversal(error))?;
            if entry.file_type().is_file() {
                let subpath = self
                    .strategy
                    .subpath(entry.path().strip_prefix(directory.as_ref()).unwrap())
                    .unwrap();
                if let Some(matches) = self.from.matches(&subpath) {
                    let source = entry.path();
                    let mut destination = source.to_path_buf();
                    destination.pop();
                    destination.push(
                        self.to
                            .resolve(source, &matches)
                            .map_err(|error| TransformError::PatternResolution(error))?,
                    );
                    policy.read(&destination)?;
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
