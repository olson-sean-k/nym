use regex::bytes::{Captures, Regex};

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
enum InnerFromPattern<'a> {
    Glob(Glob<'a>),
    Regex(Regex),
}

#[derive(Clone, Debug)]
pub struct FromPattern<'a> {
    inner: InnerFromPattern<'a>,
}

impl<'a> FromPattern<'a> {
    pub fn matches<'p>(&self, path: &'p BytePath<'_>) -> Option<Matches<'p>> {
        match self.inner {
            InnerFromPattern::Regex(ref regex) => regex.captures(path.as_ref()).map(From::from),
            InnerFromPattern::Glob(ref glob) => glob.captures(path).map(From::from),
        }
    }
}

impl<'a> From<Glob<'a>> for FromPattern<'a> {
    fn from(glob: Glob<'a>) -> Self {
        FromPattern {
            inner: InnerFromPattern::Glob(glob),
        }
    }
}

impl From<Regex> for FromPattern<'static> {
    fn from(regex: Regex) -> Self {
        FromPattern {
            inner: InnerFromPattern::Regex(regex),
        }
    }
}
