mod capture;
mod token;

use bstr::ByteVec;
use itertools::{EitherOrBoth, Itertools as _, Position};
use nom::error::ErrorKind;
use os_str_bytes::OsStrBytes as _;
use regex::bytes::Regex;
use std::borrow::{Borrow, Cow};
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{self, DirEntry, WalkDir};

use crate::glob::token::{Token, Wildcard};
use crate::PositionExt as _;

pub use crate::glob::capture::{Captures, Selector};

pub use Selector::ByIndex;
pub use Selector::ByName;

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

impl From<walkdir::Error> for GlobError {
    fn from(error: walkdir::Error) -> Self {
        GlobError::Read(error)
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

    pub fn into_owned(self) -> BytePath<'static> {
        let BytePath { path } = self;
        BytePath {
            path: path.into_owned().into(),
        }
    }

    pub fn path(&self) -> Option<Cow<Path>> {
        Path::from_bytes(self.path.as_ref()).ok()
    }
}

impl<'b> AsRef<[u8]> for BytePath<'b> {
    fn as_ref(&self) -> &[u8] {
        self.path.as_ref()
    }
}

#[derive(Debug)]
pub struct Entry {
    inner: DirEntry,
    captures: Captures<'static>,
}

impl Entry {
    pub fn into_path(self) -> PathBuf {
        self.inner.into_path()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    pub fn file_type(&self) -> FileType {
        self.inner.file_type()
    }

    pub fn depth(&self) -> usize {
        self.inner.depth()
    }

    pub fn captures(&self) -> &Captures<'static> {
        &self.captures
    }
}

#[derive(Clone, Debug)]
pub struct Glob<'t> {
    tokens: Vec<Token<'t>>,
    regex: Regex,
}

impl<'t> Glob<'t> {
    fn compile<T>(tokens: impl IntoIterator<Item = T>) -> Result<Regex, GlobError>
    where
        T: Borrow<Token<'t>>,
    {
        let mut pattern = String::new();
        let mut push = |text: &str| pattern.push_str(text);
        push("(?-u)^");
        for token in tokens.into_iter().with_position() {
            match token.interior_borrow().lift() {
                (_, Token::Literal(ref literal)) => {
                    for &byte in literal.as_bytes() {
                        push(&escape(byte));
                    }
                }
                (_, Token::NonTreeSeparator) => push(&escape(b'/')),
                (_, Token::Wildcard(Wildcard::One)) => push("([^/])"),
                (_, Token::Wildcard(Wildcard::ZeroOrMore)) => push("([^/]*)"),
                (Position::First(_), Token::Wildcard(Wildcard::Tree)) => push("(?:/?|(.*/))"),
                (Position::Middle(_), Token::Wildcard(Wildcard::Tree)) => push("(?:/|/(.*/))"),
                (Position::Last(_), Token::Wildcard(Wildcard::Tree)) => push("(?:/?|/(.*))"),
                (Position::Only(_), Token::Wildcard(Wildcard::Tree)) => push("(.*)"),
            }
        }
        push("$");
        Regex::new(&pattern).map_err(|_| GlobError::Parse)
    }

    pub fn parse(text: &'t str) -> Result<Self, GlobError> {
        let tokens: Vec<_> = token::optimize(token::parse(text)?).collect();
        let regex = Glob::compile(tokens.iter()).expect("glob compilation failed");
        Ok(Glob { tokens, regex })
    }

    pub fn into_owned(self) -> Glob<'static> {
        // Taking ownership of token data does not modify the regular
        // expression.
        let Glob { tokens, regex } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens, regex }
    }

    pub fn is_absolute(&self) -> bool {
        self.path_prefix()
            .map(|prefix| prefix.is_absolute())
            .unwrap_or(false)
    }

    pub fn has_root(&self) -> bool {
        self.path_prefix()
            .map(|prefix| prefix.has_root())
            .unwrap_or(false)
    }

    // TODO: Unix-like globs do not interact well with Windows path prefixes.
    pub fn has_prefix(&self) -> bool {
        if let Some(prefix) = self.path_prefix() {
            prefix
                .components()
                .next()
                .map(|component| matches!(component, Component::Prefix(_)))
                .unwrap_or(false)
        }
        else {
            false
        }
    }

    pub fn is_match(&self, path: impl AsRef<Path>) -> bool {
        let path = BytePath::from_path(path.as_ref());
        self.regex.is_match(&path.path)
    }

    pub fn captures<'p>(&self, path: &'p BytePath<'_>) -> Option<Captures<'p>> {
        self.regex.captures(path.as_ref()).map(From::from)
    }

    pub fn read(&self, directory: impl AsRef<Path>, depth: usize) -> Read<'_, 't> {
        // The directory tree is traversed from `root`, which may include a path
        // prefix from the glob pattern. `Read` patterns are only applied to
        // path components following the `prefix` in `root`.
        let (prefix, root) = if let Some(prefix) = self.path_prefix() {
            let root: Cow<'_, Path> = directory.as_ref().join(&prefix).into();
            if prefix.is_absolute() {
                // Note that absolute paths replace paths with which they are
                // joined, so there is no prefix.
                (PathBuf::new().into(), root)
            }
            else {
                (directory.as_ref().into(), root)
            }
        }
        else {
            let root: Cow<'_, Path> = directory.as_ref().into();
            (root.clone(), root)
        };
        let regexes = Read::compile(self.tokens.iter()).expect("glob compilation failed");
        Read {
            glob: self,
            regexes,
            prefix: prefix.into_owned(),
            walk: WalkDir::new(root)
                .follow_links(false)
                .min_depth(1)
                .max_depth(depth)
                .into_iter(),
        }
    }

    // TODO: Copies and allocations could be avoided in cases where zero or one
    //       tokens form a prefix, but this introduces complexity. Could such an
    //       optimization be worthwhile?
    fn non_wildcard_prefix(&self) -> Option<String> {
        let mut prefix = String::new();
        for token in self
            .tokens
            .iter()
            .take_while(|token| matches!(token, Token::Literal(_) | Token::NonTreeSeparator))
        {
            match *token {
                Token::Literal(ref literal) => prefix.push_str(literal.as_ref()),
                Token::NonTreeSeparator => prefix.push('/'),
                _ => {}
            }
        }
        if prefix.is_empty() {
            None
        }
        else {
            Some(prefix)
        }
    }

    fn path_prefix(&self) -> Option<PathBuf> {
        self.non_wildcard_prefix().and_then(|prefix| {
            let path = PathBuf::from(prefix);
            if self.tokens.len() > 1 {
                path.parent().map(|parent| parent.to_path_buf())
            }
            else {
                Some(path)
            }
        })
    }
}

impl FromStr for Glob<'static> {
    type Err = GlobError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Glob::parse(text).map(|glob| glob.into_owned())
    }
}

pub struct Read<'g, 't> {
    glob: &'g Glob<'t>,
    regexes: Vec<Regex>,
    prefix: PathBuf,
    walk: walkdir::IntoIter,
}

impl<'g, 't> Read<'g, 't> {
    fn compile<I, T>(tokens: I) -> Result<Vec<Regex>, GlobError>
    where
        I: IntoIterator<Item = T>,
        I::IntoIter: Clone,
        T: Borrow<Token<'t>> + Clone,
    {
        let mut regexes = Vec::new();
        let mut tokens = tokens.into_iter().peekable();
        while let Some(token) = tokens.peek().map(|token| token.borrow()) {
            match token {
                Token::NonTreeSeparator => {
                    tokens.next();
                    continue; // Skip separators.
                }
                Token::Wildcard(Wildcard::Tree) => {
                    break; // Stop at tree tokens.
                }
                _ => {
                    regexes.push(Glob::compile(tokens.take_while_ref(|token| {
                        !matches!(
                            token.borrow(),
                            Token::NonTreeSeparator | Token::Wildcard(Wildcard::Tree)
                        )
                    }))?);
                }
            }
        }
        Ok(regexes)
    }
}

impl<'g, 't> Iterator for Read<'g, 't> {
    type Item = Result<Entry, GlobError>;

    fn next(&mut self) -> Option<Self::Item> {
        'walk: while let Some(entry) = self.walk.next() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    return Some(Err(error.into()));
                }
            };
            let path = entry
                .path()
                .strip_prefix(&self.prefix)
                .expect("path is not in tree");
            for candidate in path
                .components()
                .filter_map(|component| match component {
                    Component::Normal(text) => Some(text.to_str().unwrap().as_bytes()),
                    _ => None,
                })
                .zip_longest(self.regexes.iter())
            {
                match candidate {
                    EitherOrBoth::Both(component, regex) => {
                        if regex.is_match(component) {
                            let bytes = BytePath::from_path(path);
                            if let Some(captures) = self.glob.captures(&bytes) {
                                let captures = captures.into_owned();
                                return Some(Ok(Entry {
                                    inner: entry,
                                    captures,
                                }));
                            }
                        }
                        else {
                            // Do not descend into directories that do not
                            // match the corresponding component regex.
                            if entry.file_type().is_dir() {
                                self.walk.skip_current_dir();
                            }
                            continue 'walk;
                        }
                    }
                    EitherOrBoth::Left(_) => {
                        let bytes = BytePath::from_path(path);
                        if let Some(captures) = self.glob.captures(&bytes) {
                            let captures = captures.into_owned();
                            return Some(Ok(Entry {
                                inner: entry,
                                captures,
                            }));
                        }
                    }
                    EitherOrBoth::Right(_) => {
                        continue 'walk;
                    }
                }
            }
        }
        None
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

    use crate::glob::{ByIndex, BytePath, Glob};

    // ---
    // TODO: Remove this (very broken) test. This is used for some quick and
    //       dirty manual testing, but should be removed ASAP.
    #[test]
    fn read() {
        //let glob = Glob::parse("/home/sean/src/nym/src/**/*.rs").unwrap();
        let glob = Glob::parse("src/**/*.rs").unwrap();
        eprintln!("GLOB {:?}", glob);
        for entry in glob.read(".", 255) {
            let entry = entry.unwrap();
            eprintln!("MATCHED: {:?}", entry.path());
        }
    }
    // ---

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
                .get(ByIndex(1))
                .unwrap()
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
        assert_eq!(b"a/", captures.get(ByIndex(1)).unwrap());
        assert_eq!(b"file", captures.get(ByIndex(2)).unwrap());
    }
}
