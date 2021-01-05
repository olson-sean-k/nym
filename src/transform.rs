use std::path::Path;
use thiserror::Error;
use walkdir::WalkDir;

use crate::manifest::{Manifest, ManifestError, Router};
use crate::pattern::{Candidate, FromPattern, PatternError, ToPattern};
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

#[derive(Clone, Debug)]
pub struct Transform<'f, 't> {
    pub from: FromPattern<'f>,
    pub to: ToPattern<'t>,
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
                let source = entry.path();
                let candidate = Candidate::tree(directory.as_ref(), source);
                if let Some((matches, destination)) = self.from.apply(&candidate) {
                    let mut destination = destination.to_path_buf();
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
