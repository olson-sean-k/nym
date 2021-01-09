use regex::bytes::{Captures as RegexCaptures, Regex};
use std::path::Path;

use crate::glob::{BytePath, Glob};

pub enum Selector<'t> {
    ByIndex(usize),
    ByName(&'t str),
}

#[derive(Debug)]
pub struct Captures<'a> {
    inner: RegexCaptures<'a>,
}

impl<'a> Captures<'a> {
    pub fn matched(&self) -> &'a [u8] {
        self.get(Selector::ByIndex(0)).unwrap()
    }

    pub fn get(&self, selector: Selector<'_>) -> Option<&'a [u8]> {
        match selector {
            Selector::ByIndex(index) => self.inner.get(index),
            Selector::ByName(name) => self.inner.name(name),
        }
        .map(|capture| capture.as_bytes())
    }
}

impl<'a> From<RegexCaptures<'a>> for Captures<'a> {
    fn from(captures: RegexCaptures<'a>) -> Self {
        Captures { inner: captures }
    }
}

#[derive(Clone, Debug)]
pub struct Candidate<'p> {
    source: BytePath<'p>,
    destination: &'p Path,
}

impl<'p> Candidate<'p> {
    pub fn leaf(directory: &'p Path, source: &'p Path) -> Self {
        let source = source
            .strip_prefix(directory)
            .expect("source path is not in tree");
        Candidate {
            source: BytePath::from_os_str(source.file_name().expect("source path is not a file")),
            destination: source.parent().expect("source path has no parent"),
        }
    }

    pub fn tree(directory: &'p Path, source: &'p Path) -> Self {
        let source = source
            .strip_prefix(directory)
            .expect("source path is not in tree");
        Candidate {
            source: BytePath::from_path(source),
            destination: directory,
        }
    }
}

#[derive(Clone, Debug)]
enum InnerPattern<'a> {
    Glob(Glob<'a>),
    Regex(Regex),
}

#[derive(Clone, Debug)]
pub struct FromPattern<'a> {
    inner: InnerPattern<'a>,
}

impl<'a> FromPattern<'a> {
    pub fn captures<'p>(&self, candidate: &'p Candidate<'p>) -> Option<(Captures<'p>, &'p Path)> {
        match self.inner {
            InnerPattern::Regex(ref regex) => {
                regex.captures(candidate.source.as_ref()).map(From::from)
            }
            InnerPattern::Glob(ref glob) => glob.captures(&candidate.source).map(From::from),
        }
        .map(|matches| (matches, candidate.destination))
    }
}

impl<'a> From<Glob<'a>> for FromPattern<'a> {
    fn from(glob: Glob<'a>) -> Self {
        FromPattern {
            inner: InnerPattern::Glob(glob),
        }
    }
}

impl From<Regex> for FromPattern<'static> {
    fn from(regex: Regex) -> Self {
        FromPattern {
            inner: InnerPattern::Regex(regex),
        }
    }
}
