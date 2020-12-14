use normpath::PathExt as _;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::manifest::Manifest;
use crate::pattern::{FromPattern, ToPattern};

#[derive(Clone, Debug)]
pub struct Transform<'t> {
    pub from: FromPattern,
    pub to: ToPattern<'t>,
}

impl<'t> Transform<'t> {
    pub fn read<M>(&self, directory: impl AsRef<Path>, depth: usize) -> io::Result<M>
    where
        M: Manifest,
    {
        let mut manifest = M::default();
        for entry in WalkDir::new(directory.as_ref().normalize()?)
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth)
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(find) = entry
                    .path()
                    .file_name()
                    .and_then(|name| self.from.find(name.to_str().unwrap()))
                {
                    let source = entry.path();
                    let mut destination = source.to_path_buf();
                    destination.pop();
                    destination.push(self.to.resolve(&find).unwrap()); // TODO: Do not `unwrap`.
                    let destination = if let Some(parent) = destination.parent() {
                        let parent = parent.normalize()?.into_path_buf();
                        parent.join(destination.file_name().unwrap())
                    }
                    else {
                        destination
                    };
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
