use regex::Regex;

use crate::Pattern;

pub struct Transform<'a> {
    is_recursive: bool,
    from: Regex,
    to: Pattern<'a>,
}

impl<'a> Transform<'a> {
    pub fn read(&self) {}
}
