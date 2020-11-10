use regex::Regex;
use std::io;

use crate::pattern::Pattern;

pub struct Move<'a> {
    from: Regex,
    to: Pattern<'a>,
}

impl<'a> Move<'a> {
    pub fn new(from: Regex, to: Pattern<'a>) -> Self {
        Move { from, to }
    }

    pub fn execute(&mut self) -> io::Result<()> {
        Ok(())
    }
}
