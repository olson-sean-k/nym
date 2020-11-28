use bimap::BiMap;
use normpath::PathExt as _;
use regex::Regex;
use std::io::{self, Error, ErrorKind};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::pattern::{Capture, Component, Pattern};

pub trait Manifest: Default + IntoIterator<Item = (PathBuf, PathBuf)> {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()>;
}

impl Manifest for BiMap<PathBuf, PathBuf> {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()> {
        self.insert_no_overwrite(source.into(), destination.into())
            .map_err(|_| Error::from(ErrorKind::Other))
    }
}

pub struct Transform<'a> {
    pub from: Regex,
    pub to: Pattern<'a>,
}

impl<'a> Transform<'a> {
    pub fn scan<M>(&self, directory: impl AsRef<Path>, depth: usize) -> io::Result<M>
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
                if let Some(captures) = entry
                    .path()
                    .file_name()
                    .and_then(|name| self.from.captures(name.to_str().unwrap()))
                {
                    let source = entry.path();
                    let mut destination = entry.path().to_path_buf();
                    destination.pop();
                    let mut head = String::new();
                    for component in self.to.components() {
                        match component {
                            Component::Capture(capture) => match capture {
                                Capture::Index(index) => {
                                    head.push_str(captures.get(*index).unwrap().as_str());
                                }
                                Capture::Name(name) => {
                                    head.push_str(captures.name(name).unwrap().as_str());
                                }
                            },
                            Component::Literal(text) => {
                                head.push_str(text);
                            }
                        }
                    }
                    destination.push(head);
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
