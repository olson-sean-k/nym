use regex::bytes::{Captures, Regex};
use std::path::Path;

use crate::glob::{BytePath, Glob};

pub enum Selector<'t> {
    ByIndex(usize),
    ByName(&'t str),
}

#[derive(Debug)]
pub struct Matches<'a> {
    captures: Captures<'a>,
}

impl<'a> Matches<'a> {
    pub fn get(&self) -> &'a [u8] {
        self.capture(Selector::ByIndex(0)).unwrap()
    }

    pub fn capture(&self, selector: Selector<'_>) -> Option<&'a [u8]> {
        match selector {
            Selector::ByIndex(index) => self.captures.get(index),
            Selector::ByName(name) => self.captures.name(name),
        }
        .map(|capture| capture.as_bytes())
    }
}

impl<'a> From<Captures<'a>> for Matches<'a> {
    fn from(captures: Captures<'a>) -> Self {
        Matches { captures }
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
enum Format<'a> {
    Glob(Glob<'a>),
    Regex(Regex),
}

#[derive(Clone, Debug)]
pub struct FromPattern<'a> {
    format: Format<'a>,
}

impl<'a> FromPattern<'a> {
    pub fn apply<'p>(&self, candidate: &'p Candidate<'p>) -> Option<(Matches<'p>, &'p Path)> {
        match self.format {
            Format::Regex(ref regex) => regex.captures(candidate.source.as_ref()).map(From::from),
            Format::Glob(ref glob) => glob.captures(&candidate.source).map(From::from),
        }
        .map(|matches| (matches, candidate.destination))
    }
}

impl<'a> From<Glob<'a>> for FromPattern<'a> {
    fn from(glob: Glob<'a>) -> Self {
        FromPattern {
            format: Format::Glob(glob),
        }
    }
}

impl From<Regex> for FromPattern<'static> {
    fn from(regex: Regex) -> Self {
        FromPattern {
            format: Format::Regex(regex),
        }
    }
}
