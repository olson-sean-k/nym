use bimap::BiMap;
use regex::Regex;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::{Capture, Component, Pattern};

pub struct Transform<'a> {
    pub from: Regex,
    pub to: Pattern<'a>,
}

impl<'a> Transform<'a> {
    pub fn scan(&self, directory: impl AsRef<Path>) -> io::Result<BiMap<PathBuf, PathBuf>> {
        let mut renames = BiMap::new();
        for entry in WalkDir::new(directory).follow_links(false).max_depth(1) {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(captures) = entry
                    .path()
                    .file_name()
                    .and_then(|name| self.from.captures(name.to_str().unwrap()))
                {
                    let source = entry.path().canonicalize()?;
                    let mut destination = source.clone();
                    destination.pop();
                    let mut head = String::new();
                    for component in self.to.components.iter() {
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
                    renames
                        .insert_no_overwrite(source, destination)
                        .expect("redundant");
                }
            }
        }
        Ok(renames)
    }
}
