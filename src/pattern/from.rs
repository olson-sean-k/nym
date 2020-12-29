use regex::{Captures, Regex};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::glob::Glob;
use crate::pattern::{PatternError, ToPattern};

pub enum Selector<'t> {
    ByIndex(usize),
    ByName(&'t str),
}

#[derive(Debug)]
enum InnerFind<'t> {
    Glob,
    Regex(Captures<'t>),
}

#[derive(Debug)]
pub struct Find<'t> {
    inner: InnerFind<'t>,
}

impl<'t> Find<'t> {
    pub fn capture(&self, selector: Selector<'_>) -> Option<&'t str> {
        match self.inner {
            InnerFind::Regex(ref captures) => match selector {
                Selector::ByIndex(index) => captures.get(index),
                Selector::ByName(name) => captures.name(name),
            }
            .map(|capture| capture.as_str()),
            _ => todo!(),
        }
    }
}

impl<'t> From<Captures<'t>> for Find<'t> {
    fn from(captures: Captures<'t>) -> Self {
        Find {
            inner: InnerFind::Regex(captures),
        }
    }
}

#[derive(Clone, Debug)]
enum InnerPattern {
    Glob(Glob),
    Regex(Regex),
}

#[derive(Clone, Debug)]
pub struct FromPattern {
    inner: InnerPattern,
}

impl FromPattern {
    pub fn find<'t>(&self, text: &'t str) -> Option<Find<'t>> {
        match self.inner {
            InnerPattern::Regex(ref regex) => regex.captures(text).map(From::from),
            _ => todo!(),
        }
    }

    pub(in crate) fn read<'p>(
        &'p self,
        to: &'p ToPattern,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Box<dyn 'p + Iterator<Item = Result<(PathBuf, PathBuf), PatternError>>> {
        let walk = WalkDir::new(directory.as_ref())
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth);
        match self.inner {
            InnerPattern::Regex(ref regex) => Box::new(
                walk.into_iter()
                    .map(|entry| entry.map_err(|error| PatternError::ReadTree(error)))
                    .filter(|entry| {
                        entry
                            .as_ref()
                            .map(|entry| entry.file_type().is_file())
                            .unwrap_or(true)
                    })
                    .flat_map(move |entry| {
                        entry
                            .and_then(move |entry| {
                                let source = entry.path();
                                let mut destination = source.to_path_buf();
                                destination.pop();
                                source
                                    .file_name()
                                    .and_then(|name| name.to_str())
                                    .and_then(|name| regex.captures(name).map(Find::from))
                                    .as_ref()
                                    .map(move |find| {
                                        to.resolve(source, find).map(move |name| {
                                            destination.push(name);
                                            (source.to_path_buf(), destination)
                                        })
                                    })
                                    .transpose()
                            })
                            .transpose()
                    }),
            ),
            _ => todo!(),
        }
    }
}

impl From<Glob> for FromPattern {
    fn from(glob: Glob) -> FromPattern {
        FromPattern {
            inner: InnerPattern::Glob(glob),
        }
    }
}

impl From<Regex> for FromPattern {
    fn from(regex: Regex) -> FromPattern {
        FromPattern {
            inner: InnerPattern::Regex(regex),
        }
    }
}
