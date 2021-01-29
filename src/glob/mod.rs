mod token;

use bstr::ByteVec;
use itertools::{Itertools as _, Position};
use nom::error::ErrorKind;
use regex::bytes::Regex;
use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{self, WalkDir};

use crate::glob::token::{Token, Wildcard};
use crate::PositionExt as _;

pub use regex::bytes::Captures;
use std::ffi::OsStr;

#[derive(Debug, Error)]
pub enum GlobError {
    #[error("failed to parse glob")]
    Parse,
    #[error("failed to read directory tree")]
    Read(walkdir::Error),
}

impl<I> From<nom::Err<(I, ErrorKind)>> for GlobError {
    fn from(_: nom::Err<(I, ErrorKind)>) -> Self {
        GlobError::Parse
    }
}

#[derive(Clone, Debug)]
pub struct BytePath<'b> {
    path: Cow<'b, [u8]>,
}

impl<'b> BytePath<'b> {
    fn from_bytes(bytes: Cow<'b, [u8]>) -> Self {
        #[cfg(unix)]
        fn normalize(path: Cow<[u8]>) -> Cow<[u8]> {
            path
        }

        #[cfg(not(unix))]
        fn normalize(mut path: Cow<[u8]>) -> Cow<[u8]> {
            use std::path;

            for i in 0..path.len() {
                if path[i] == b'/' || !path::is_separator(path[i] as char) {
                    continue;
                }
                path.to_mut()[i] = b'/';
            }
            path
        }

        let path = normalize(bytes);
        BytePath { path }
    }

    pub fn from_os_str(text: &'b OsStr) -> Self {
        Self::from_bytes(Vec::from_os_str_lossy(text))
    }

    pub fn from_path<P>(path: &'b P) -> Self
    where
        P: AsRef<Path> + ?Sized,
    {
        Self::from_bytes(Vec::from_path_lossy(path.as_ref()))
    }
}

impl<'b> AsRef<[u8]> for BytePath<'b> {
    fn as_ref(&self) -> &[u8] {
        self.path.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct Glob<'t> {
    tokens: Vec<Token<'t>>,
    regex: Regex,
}

impl<'t> Glob<'t> {
    fn from_tokens<I>(tokens: I) -> Result<Self, GlobError>
    where
        I: IntoIterator<Item = Token<'t>>,
    {
        let tokens: Vec<_> = token::coalesce(tokens).collect();
        let mut pattern = String::new();
        let mut push = |text: &str| pattern.push_str(text);
        push("(?-u)^");
        for token in tokens.iter().with_position() {
            match token.lift() {
                (_, Token::Literal(ref literal)) => {
                    for &byte in literal.as_bytes() {
                        push(&escape(byte));
                    }
                }
                (_, Token::NonTreeSeparator) => push(&escape(b'/')),
                (_, Token::Wildcard(Wildcard::One)) => push("([^/])"),
                (_, Token::Wildcard(Wildcard::ZeroOrMore)) => push("([^/]*)"),
                (Position::First(()), Token::Wildcard(Wildcard::Tree)) => push("(?:/?|(.*/))"),
                (Position::Middle(()), Token::Wildcard(Wildcard::Tree)) => push("(?:/|/(.*/))"),
                (Position::Last(()), Token::Wildcard(Wildcard::Tree)) => push("(?:/?|/(.*))"),
                (Position::Only(()), Token::Wildcard(Wildcard::Tree)) => push("(.*)"),
            }
        }
        push("$");
        let regex = Regex::new(&pattern).map_err(|_| GlobError::Parse)?;
        Ok(Glob { tokens, regex })
    }

    pub fn parse(text: &'t str) -> Result<Self, GlobError> {
        token::parse(text).and_then(Glob::from_tokens)
    }

    pub fn into_owned(self) -> Glob<'static> {
        // Taking ownership of token data does not modify the regular
        // expression.
        let Glob { tokens, regex } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens, regex }
    }

    pub fn is_absolute(&self) -> bool {
        self.non_wildcard_prefix()
            .map(|literal| {
                let path = Path::new(literal.as_ref());
                path.is_absolute()
            })
            .unwrap_or(false)
    }

    pub fn has_root(&self) -> bool {
        self.non_wildcard_prefix()
            .map(|literal| {
                let path = Path::new(literal.as_ref());
                path.has_root()
            })
            .unwrap_or(false)
    }

    // TODO: Unix-like globs do not interact well with Windows path prefixes.
    pub fn has_prefix(&self) -> bool {
        self.non_wildcard_prefix()
            .and_then(|literal| {
                let path = Path::new(literal.as_ref());
                path.components().next()
            })
            .map(|component| matches!(component, Component::Prefix(_)))
            .unwrap_or(false)
    }

    pub fn is_match(&self, path: impl AsRef<Path>) -> bool {
        let path = BytePath::from_path(path.as_ref());
        self.regex.is_match(&path.path)
    }

    pub fn captures<'p>(&self, path: &'p BytePath<'_>) -> Option<Captures<'p>> {
        self.regex.captures(path.as_ref())
    }

    pub fn read(
        text: &'t str,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Read<'t>, GlobError> {
        let glob = Glob::parse(text)?;
        let directory = if let Some(prefix) = glob.path_prefix() {
            directory.as_ref().join(prefix)
        }
        else {
            directory.as_ref().to_path_buf()
        };
        let components = Path::new(text)
            .components()
            .flat_map(|component| match component {
                Component::Normal(text) => Glob::parse(text.to_str().unwrap()).ok(),
                _ => None,
            })
            .take_while(|glob| !glob.is_any_tree())
            .collect();
        Ok(Read {
            glob,
            components,
            walk: WalkDir::new(directory)
                .follow_links(false)
                .min_depth(1)
                .max_depth(depth)
                .into_iter(),
        })
    }

    fn non_wildcard_prefix(&self) -> Option<&Cow<'t, str>> {
        self.tokens.get(0).and_then(|token| match *token {
            Token::Literal(ref literal) => Some(literal),
            _ => None,
        })
    }

    fn path_prefix(&self) -> Option<&Path> {
        self.non_wildcard_prefix().and_then(|literal| {
            let path = Path::new(literal.as_ref());
            if self.tokens.len() > 1 {
                path.parent()
            }
            else {
                Some(path)
            }
        })
    }

    fn is_any_tree(&self) -> bool {
        self.tokens.len() == 1
            && matches!(self.tokens.get(0).unwrap(), Token::Wildcard(Wildcard::Tree))
    }
}

impl FromStr for Glob<'static> {
    type Err = GlobError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Glob::parse(text).map(|glob| glob.into_owned())
    }
}

pub struct Read<'t> {
    glob: Glob<'t>,
    components: Vec<Glob<'t>>,
    //strip: PathBuf,
    walk: walkdir::IntoIter,
}

impl<'t> Read<'t> {
    pub fn glob(&self) -> &Glob<'t> {
        &self.glob
    }
}

impl<'t> Iterator for Read<'t> {
    type Item = Result<PathBuf, GlobError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(entry) = self.walk.next() {
            let entry = entry.unwrap(); // TODO: Forward errors.
            let path = entry.path();

            None // TODO:
        }
        else {
            None
        }
    }
}

fn escape(byte: u8) -> String {
    const ASCII_TERMINATOR: u8 = 0x7F;

    if byte <= ASCII_TERMINATOR {
        regex::escape(&(byte as char).to_string())
    }
    else {
        format!("\\x{:02x}", byte)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::glob::{BytePath, Glob};

    #[test]
    fn parse_glob_with_zom_tokens() {
        Glob::parse("*").unwrap();
        Glob::parse("a/*").unwrap();
        Glob::parse("*a").unwrap();
        Glob::parse("a*").unwrap();
        Glob::parse("a*b").unwrap();
        Glob::parse("/*").unwrap();
    }

    #[test]
    fn parse_glob_with_one_tokens() {
        Glob::parse("?").unwrap();
        Glob::parse("a/?").unwrap();
        Glob::parse("?a").unwrap();
        Glob::parse("a?").unwrap();
        Glob::parse("a?b").unwrap();
        Glob::parse("??a??b??").unwrap();
        Glob::parse("/?").unwrap();
    }

    #[test]
    fn parse_glob_with_one_and_zom_tokens() {
        Glob::parse("?*").unwrap();
        Glob::parse("*?").unwrap();
        Glob::parse("*/?").unwrap();
        Glob::parse("?*?").unwrap();
        Glob::parse("/?*").unwrap();
    }

    #[test]
    fn parse_glob_with_tree_tokens() {
        Glob::parse("**").unwrap();
        Glob::parse("**/").unwrap();
        Glob::parse("/**").unwrap();
        Glob::parse("**/a").unwrap();
        Glob::parse("a/**").unwrap();
        Glob::parse("**/a/**/b/**").unwrap();
        Glob::parse("**/**/a").unwrap();
    }

    #[test]
    fn reject_glob_with_adjacent_tree_or_zom_tokens() {
        assert!(Glob::parse("***").is_err());
        assert!(Glob::parse("****").is_err());
        assert!(Glob::parse("**/*/***").is_err());
    }

    #[test]
    fn reject_glob_with_adjacent_literal_tokens() {
        assert!(Glob::parse("**a").is_err());
        assert!(Glob::parse("a**").is_err());
        assert!(Glob::parse("a**b").is_err());
        assert!(Glob::parse("a*b**").is_err());
        assert!(Glob::parse("**/**a/**").is_err());
    }

    #[test]
    fn reject_glob_with_adjacent_one_tokens() {
        assert!(Glob::parse("**?").is_err());
        assert!(Glob::parse("?**").is_err());
        assert!(Glob::parse("?**?").is_err());
        assert!(Glob::parse("?*?**").is_err());
        assert!(Glob::parse("**/**?/**").is_err());
    }

    #[test]
    fn match_glob_with_tree_tokens() {
        let glob = Glob::parse("a/**/b").unwrap();

        assert!(glob.is_match(Path::new("a/b")));
        assert!(glob.is_match(Path::new("a/x/b")));
        assert!(glob.is_match(Path::new("a/x/y/z/b")));

        assert!(!glob.is_match(Path::new("a")));
        assert!(!glob.is_match(Path::new("b/a")));

        assert_eq!(
            b"x/y/z/",
            glob.captures(&BytePath::from_path(Path::new("a/x/y/z/b")))
                .unwrap()
                .get(1)
                .unwrap()
                .as_bytes()
        );
    }

    #[test]
    fn match_glob_with_tree_and_zom_tokens() {
        let glob = Glob::parse("**/*.ext").unwrap();

        assert!(glob.is_match(Path::new("file.ext")));
        assert!(glob.is_match(Path::new("a/file.ext")));
        assert!(glob.is_match(Path::new("a/b/file.ext")));

        let path = BytePath::from_path(Path::new("a/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"a/", captures.get(1).unwrap().as_bytes());
        assert_eq!(b"file", captures.get(2).unwrap().as_bytes());
    }
}
