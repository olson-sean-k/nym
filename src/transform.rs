use normpath::PathExt as _;
use std::io;
use std::path::Path;
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
                    destination.push(self.to.join(&find).unwrap()); // TODO:
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
