use std::path::Path;
use thiserror::Error;
use walkdir::WalkDir;

use crate::manifest::{Manifest, ManifestError, Routing};
use crate::pattern::{FromPattern, PatternError, ToPattern};

#[derive(Debug, Error)]
pub enum TransformError {
    #[error("failed to traverse directory tree: {0}")]
    DirectoryTraversal(walkdir::Error),
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

#[derive(Clone, Debug)]
pub struct Transform<'t> {
    pub from: FromPattern,
    pub to: ToPattern<'t>,
}

impl<'t> Transform<'t> {
    pub fn read<M>(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Manifest<M>, TransformError>
    where
        M: Routing,
    {
        let mut manifest = Manifest::default();
        for entry in WalkDir::new(directory.as_ref())
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth)
        {
            let entry = entry.map_err(|error| TransformError::DirectoryTraversal(error))?;
            if entry.file_type().is_file() {
                if let Some(find) = entry
                    .path()
                    .file_name()
                    .and_then(|name| self.from.find(name.to_str().unwrap()))
                {
                    let source = entry.path();
                    let mut destination = source.to_path_buf();
                    destination.pop();
                    destination.push(
                        self.to
                            .resolve(source, &find)
                            .map_err(|error| TransformError::PatternResolution(error))?,
                    );
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
