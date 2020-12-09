use regex::{Captures, Regex};

use crate::glob::Glob;

pub enum Selector<'t> {
    ByIndex(usize),
    ByName(&'t str),
}

enum InnerFind<'t> {
    Glob,
    Regex(Captures<'t>),
}

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

enum InnerPattern {
    Glob(Glob),
    Regex(Regex),
}

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
