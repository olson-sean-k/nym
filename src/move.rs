use regex::Regex;
use std::io;

use crate::pattern::to::ToPattern;

pub struct Move<'a> {
    from: Regex,
    to: ToPattern<'a>,
}

impl<'a> Move<'a> {
    pub fn new(from: Regex, to: ToPattern<'a>) -> Self {
        Move { from, to }
    }

    pub fn execute(&mut self) -> io::Result<()> {
        Ok(())
    }
}
