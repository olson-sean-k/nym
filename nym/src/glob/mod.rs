mod capture;
mod rule;
mod token;

use bstr::ByteVec;
use itertools::{EitherOrBoth, Itertools as _, Position};
use nom::error::ErrorKind;
use os_str_bytes::OsStrBytes as _;
use regex::bytes::Regex;
use std::borrow::{Borrow, Cow};
use std::convert::TryFrom;
use std::ffi::OsStr;
use std::fs::{FileType, Metadata};
use std::iter::Fuse;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{self, DirEntry, WalkDir};

use crate::glob::token::{Token, Wildcard};

pub use crate::glob::capture::Captures;
pub use crate::glob::rule::RuleError;

trait IteratorExt: Iterator + Sized {
    fn adjacent(self) -> Adjacent<Self>
    where
        Self::Item: Clone;
}

impl<I> IteratorExt for I
where
    I: Iterator,
{
    fn adjacent(self) -> Adjacent<Self>
    where
        Self::Item: Clone,
    {
        Adjacent::new(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Adjacency<T> {
    Only(T),
    First(T, T),
    Middle(T, T, T),
    Last(T, T),
}

impl<T> Adjacency<T> {
    pub fn into_tuple(self) -> (Option<T>, T, Option<T>) {
        match self {
            Adjacency::Only(item) => (None, item, None),
            Adjacency::First(item, right) => (None, item, Some(right)),
            Adjacency::Middle(left, item, right) => (Some(left), item, Some(right)),
            Adjacency::Last(left, item) => (Some(left), item, None),
        }
    }
}

struct Adjacent<I>
where
    I: Iterator,
{
    input: Fuse<I>,
    adjacency: Option<Adjacency<I::Item>>,
}

impl<I> Adjacent<I>
where
    I: Iterator,
{
    fn new(input: I) -> Self {
        let mut input = input.fuse();
        let adjacency = match (input.next(), input.next()) {
            (Some(first), Some(second)) => Some(Adjacency::First(first, second)),
            (Some(first), None) => Some(Adjacency::Only(first)),
            (None, None) => None,
            // The input iterator is fused, so this cannot occur.
            (None, Some(_)) => unreachable!(),
        };
        Adjacent { input, adjacency }
    }
}

impl<I> Iterator for Adjacent<I>
where
    I: Iterator,
    I::Item: Clone,
{
    type Item = Adjacency<I::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.input.next();
        self.adjacency.take().map(|adjacency| {
            self.adjacency = match adjacency.clone() {
                Adjacency::First(left, item) | Adjacency::Middle(_, left, item) => {
                    if let Some(right) = next {
                        Some(Adjacency::Middle(left, item, right))
                    }
                    else {
                        Some(Adjacency::Last(left, item))
                    }
                }
                Adjacency::Only(_) | Adjacency::Last(_, _) => None,
            };
            adjacency
        })
    }
}

trait PositionExt<T> {
    fn as_tuple(&self) -> (Position<()>, &T);

    fn interior_borrow<B>(&self) -> Position<&B>
    where
        T: Borrow<B>;
}

impl<T> PositionExt<T> for Position<T> {
    fn as_tuple(&self) -> (Position<()>, &T) {
        match *self {
            Position::First(ref inner) => (Position::First(()), inner),
            Position::Middle(ref inner) => (Position::Middle(()), inner),
            Position::Last(ref inner) => (Position::Last(()), inner),
            Position::Only(ref inner) => (Position::Only(()), inner),
        }
    }

    fn interior_borrow<B>(&self) -> Position<&B>
    where
        T: Borrow<B>,
    {
        match *self {
            Position::First(ref inner) => Position::First(inner.borrow()),
            Position::Middle(ref inner) => Position::Middle(inner.borrow()),
            Position::Last(ref inner) => Position::Last(inner.borrow()),
            Position::Only(ref inner) => Position::Only(inner.borrow()),
        }
    }
}

trait SliceExt<T> {
    fn terminals(&self) -> Option<Terminals<&T>>;
}

impl<T> SliceExt<T> for [T] {
    fn terminals(&self) -> Option<Terminals<&T>> {
        match self.len() {
            0 => None,
            1 => Some(Terminals::Only(&self[0])),
            _ => Some(Terminals::StartEnd(
                self.first().unwrap(),
                self.last().unwrap(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Terminals<T> {
    Only(T),
    StartEnd(T, T),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GlobError {
    #[error("failed to parse glob: {0}")]
    Parse(nom::Err<(String, ErrorKind)>),
    #[error("invalid glob: {0}")]
    Rule(RuleError),
    #[error("failed to read directory tree: {0}")]
    Read(walkdir::Error),
}

impl<'i> From<nom::Err<(&'i str, ErrorKind)>> for GlobError {
    fn from(error: nom::Err<(&'i str, ErrorKind)>) -> Self {
        GlobError::Parse(error.to_owned())
    }
}

impl From<walkdir::Error> for GlobError {
    fn from(error: walkdir::Error) -> Self {
        GlobError::Read(error)
    }
}

impl From<RuleError> for GlobError {
    fn from(error: RuleError) -> Self {
        GlobError::Rule(error)
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

        // NOTE: This doesn't consider platforms where `/` is not a path
        //       separator or is otherwise supported in file and directory
        //       names. `/` and `\` are by far the most common separators
        //       (including mixed-mode operation as seen in Windows), but there
        //       is precedence for alternatives like `>`, `.`, and `:`.
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
        Path::from_raw_bytes(self.path.as_ref()).ok()
    }
}

impl<'b> AsRef<[u8]> for BytePath<'b> {
    fn as_ref(&self) -> &[u8] {
        self.path.as_ref()
    }
}

#[derive(Debug)]
pub struct Entry<'t> {
    inner: DirEntry,
    captures: Captures<'t>,
}

impl<'t> Entry<'t> {
    pub fn into_path(self) -> PathBuf {
        self.inner.into_path()
    }

    pub fn path(&self) -> &Path {
        self.inner.path()
    }

    pub fn file_type(&self) -> FileType {
        self.inner.file_type()
    }

    // TODO: On some platforms, traversing a directory tree also yields file
    //       metadata (e.g., Windows). Forward this metadata to path printing
    //       using `lscolors` in `nym-cli` to avoid unnecessary reads.
    pub fn metadata(&self) -> Result<Metadata, GlobError> {
        self.inner.metadata().map_err(From::from)
    }

    pub fn depth(&self) -> usize {
        self.inner.depth()
    }

    pub fn captures(&self) -> &Captures<'t> {
        &self.captures
    }
}

#[derive(Clone, Debug)]
pub struct Glob<'t> {
    tokens: Vec<Token<'t>>,
    regex: Regex,
}

impl<'t> Glob<'t> {
    fn compile<T>(tokens: impl IntoIterator<Item = T>) -> Regex
    where
        T: Borrow<Token<'t>>,
    {
        #[derive(Clone, Copy, Debug)]
        enum Grouping {
            Capture,
            NonCapture,
        }

        impl Grouping {
            pub fn push_str(&self, pattern: &mut String, encoding: &str) {
                self.push_with(pattern, || encoding.into());
            }

            pub fn push_with<'p, F>(&self, pattern: &mut String, f: F)
            where
                F: Fn() -> Cow<'p, str>,
            {
                match self {
                    Grouping::Capture => pattern.push('('),
                    Grouping::NonCapture => pattern.push_str("(?:"),
                }
                pattern.push_str(f().as_ref());
                pattern.push(')');
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

        fn encode<'t, T>(
            grouping: Grouping,
            pattern: &mut String,
            tokens: impl IntoIterator<Item = T>,
        ) where
            T: Borrow<Token<'t>>,
        {
            use itertools::Position::{First, Last, Middle, Only};

            use crate::glob::token::Archetype::{Character, Range};
            use crate::glob::token::Evaluation::{Eager, Lazy};
            use crate::glob::token::Token::{Alternative, Class, Literal, Separator, Wildcard};
            use crate::glob::token::Wildcard::{One, Tree, ZeroOrMore};

            for token in tokens.into_iter().with_position() {
                match token.interior_borrow().as_tuple() {
                    (_, Literal(ref literal)) => {
                        for &byte in literal.as_bytes() {
                            pattern.push_str(&escape(byte));
                        }
                    }
                    (_, Separator) => pattern.push_str(&escape(b'/')),
                    (_, Alternative(alternative)) => {
                        let encodings: Vec<_> = alternative
                            .branches()
                            .iter()
                            .map(|tokens| {
                                let mut pattern = String::new();
                                pattern.push_str("(?:");
                                encode(Grouping::NonCapture, &mut pattern, tokens.iter());
                                pattern.push(')');
                                pattern
                            })
                            .collect();
                        grouping.push_str(pattern, &encodings.join("|"));
                    }
                    (
                        _,
                        Class {
                            is_negated,
                            archetypes,
                        },
                    ) => {
                        grouping.push_with(pattern, || {
                            let mut pattern = String::new();
                            pattern.push('[');
                            if *is_negated {
                                pattern.push('^');
                            }
                            for archetype in archetypes {
                                match archetype {
                                    Character(literal) => {
                                        let mut bytes = [0u8; 4];
                                        literal.encode_utf8(&mut bytes);
                                        for &byte in &bytes {
                                            pattern.push_str(&escape(byte))
                                        }
                                    }
                                    Range(left, right) => {
                                        pattern.push(*left);
                                        pattern.push('-');
                                        pattern.push(*right);
                                    }
                                }
                            }
                            pattern.push_str("&&[^/]]");
                            pattern.into()
                        });
                    }
                    (_, Wildcard(One)) => grouping.push_str(pattern, "[^/]"),
                    (_, Wildcard(ZeroOrMore(Eager))) => grouping.push_str(pattern, "[^/]*"),
                    (_, Wildcard(ZeroOrMore(Lazy))) => grouping.push_str(pattern, "[^/]*?"),
                    (First(_), Wildcard(Tree)) => {
                        pattern.push_str("(?:/?|");
                        grouping.push_str(pattern, ".*/");
                        pattern.push(')');
                    }
                    (Middle(_), Wildcard(Tree)) => {
                        pattern.push_str("(?:/|/");
                        grouping.push_str(pattern, ".*/");
                        pattern.push(')');
                    }
                    (Last(_), Wildcard(Tree)) => {
                        pattern.push_str("(?:/?|/");
                        grouping.push_str(pattern, ".*");
                        pattern.push(')');
                    }
                    (Only(_), Wildcard(Tree)) => grouping.push_str(pattern, ".*"),
                }
            }
        }

        let mut pattern = String::new();
        pattern.push_str("(?-u)^");
        encode(Grouping::Capture, &mut pattern, tokens);
        pattern.push('$');
        Regex::new(&pattern).expect("glob compilation failed")
    }

    pub fn new(text: &'t str) -> Result<Self, GlobError> {
        let tokens: Vec<_> = token::optimize(token::parse(text)?).collect();
        rule::check(tokens.iter())?;
        let regex = Glob::compile(tokens.iter());
        Ok(Glob { tokens, regex })
    }

    pub fn partitioned(text: &'t str) -> Result<(PathBuf, Self), GlobError> {
        pub fn literal_prefix_upper_bound(tokens: &[Token]) -> usize {
            let mut index = 0;
            for (n, token) in tokens.iter().enumerate() {
                match token {
                    Token::Separator => {
                        index = n;
                    }
                    Token::Literal(_) => {
                        continue;
                    }
                    Token::Wildcard(Wildcard::Tree) => {
                        return n;
                    }
                    _ => {
                        return if index == 0 { index } else { index + 1 };
                    }
                }
            }
            tokens.len()
        }

        let mut tokens: Vec<_> = token::optimize(token::parse(text)?).collect();
        rule::check(tokens.iter())?;
        let prefix = token::literal_path_prefix(tokens.iter()).unwrap_or_else(PathBuf::new);
        tokens.drain(0..literal_prefix_upper_bound(&tokens));
        let regex = Glob::compile(tokens.iter());
        Ok((prefix, Glob { tokens, regex }))
    }

    pub fn into_owned(self) -> Glob<'static> {
        let Glob { tokens, regex } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens, regex }
    }

    pub fn is_absolute(&self) -> bool {
        token::literal_path_prefix(self.tokens.iter())
            .map(|prefix| prefix.is_absolute())
            .unwrap_or(false)
    }

    pub fn has_root(&self) -> bool {
        token::literal_path_prefix(self.tokens.iter())
            .map(|prefix| prefix.has_root())
            .unwrap_or(false)
    }

    pub fn is_match(&self, path: impl AsRef<Path>) -> bool {
        let path = BytePath::from_path(path.as_ref());
        self.regex.is_match(&path.path)
    }

    pub fn captures<'p>(&self, path: &'p BytePath<'_>) -> Option<Captures<'p>> {
        self.regex.captures(path.as_ref()).map(From::from)
    }

    pub fn read(
        &self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> impl '_ + Iterator<Item = Result<Entry<'static>, GlobError>> {
        // The directory tree is traversed from `root`, which may include a path
        // prefix from the glob pattern. `Read` patterns are only applied to
        // path components following the `prefix` in `root`.
        let (prefix, root) = if let Some(prefix) = token::literal_path_prefix(self.tokens.iter()) {
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
        let regexes = Read::compile(self.tokens.iter());
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
}

impl<'t> TryFrom<&'t str> for Glob<'t> {
    type Error = GlobError;

    fn try_from(text: &'t str) -> Result<Self, Self::Error> {
        Glob::new(text)
    }
}

impl FromStr for Glob<'static> {
    type Err = GlobError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Glob::new(text).map(|glob| glob.into_owned())
    }
}

struct Read<'g, 't> {
    glob: &'g Glob<'t>,
    regexes: Vec<Regex>,
    prefix: PathBuf,
    walk: walkdir::IntoIter,
}

impl<'g, 't> Read<'g, 't> {
    fn compile<I>(tokens: I) -> Vec<Regex>
    where
        I: IntoIterator<Item = &'t Token<'t>>,
        I::IntoIter: Clone,
    {
        let mut regexes = Vec::new();
        for component in token::components(tokens) {
            if component.tokens().iter().any(|token| match token {
                Token::Alternative(ref alternative) => alternative.has_component_boundary(),
                token => token.is_component_boundary(),
            }) {
                // Stop at component boundaries, such as tree wildcards or any
                // boundary within an alternative token.
                break;
            }
            else {
                regexes.push(Glob::compile(component.tokens().iter().cloned()));
            }
        }
        regexes
    }
}

impl<'g, 't> Iterator for Read<'g, 't> {
    type Item = Result<Entry<'static>, GlobError>;

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::glob::{Adjacency, BytePath, Glob, IteratorExt as _};

    #[test]
    fn adjacent() {
        let mut adjacent = Option::<i32>::None.into_iter().adjacent();
        assert_eq!(adjacent.next(), None);

        let mut adjacent = Some(0i32).into_iter().adjacent();
        assert_eq!(adjacent.next(), Some(Adjacency::Only(0)));
        assert_eq!(adjacent.next(), None);

        let mut adjacent = (0i32..3).adjacent();
        assert_eq!(adjacent.next(), Some(Adjacency::First(0, 1)));
        assert_eq!(adjacent.next(), Some(Adjacency::Middle(0, 1, 2)));
        assert_eq!(adjacent.next(), Some(Adjacency::Last(1, 2)));
        assert_eq!(adjacent.next(), None);
    }

    #[test]
    fn build_glob_with_eager_zom_tokens() {
        Glob::new("*").unwrap();
        Glob::new("a/*").unwrap();
        Glob::new("*a").unwrap();
        Glob::new("a*").unwrap();
        Glob::new("a*b").unwrap();
        Glob::new("/*").unwrap();
    }

    #[test]
    fn build_glob_with_lazy_zom_tokens() {
        Glob::new("$").unwrap();
        Glob::new("a/$").unwrap();
        Glob::new("$a").unwrap();
        Glob::new("a$").unwrap();
        Glob::new("a$b").unwrap();
        Glob::new("/$").unwrap();
    }

    #[test]
    fn build_glob_with_one_tokens() {
        Glob::new("?").unwrap();
        Glob::new("a/?").unwrap();
        Glob::new("?a").unwrap();
        Glob::new("a?").unwrap();
        Glob::new("a?b").unwrap();
        Glob::new("??a??b??").unwrap();
        Glob::new("/?").unwrap();
    }

    #[test]
    fn build_glob_with_one_and_zom_tokens() {
        Glob::new("?*").unwrap();
        Glob::new("*?").unwrap();
        Glob::new("*/?").unwrap();
        Glob::new("?*?").unwrap();
        Glob::new("/?*").unwrap();
        Glob::new("?$").unwrap();
    }

    #[test]
    fn build_glob_with_tree_tokens() {
        Glob::new("**").unwrap();
        Glob::new("**/").unwrap();
        Glob::new("/**").unwrap();
        Glob::new("**/a").unwrap();
        Glob::new("a/**").unwrap();
        Glob::new("**/a/**/b/**").unwrap();
        Glob::new("**/**/a").unwrap();
    }

    #[test]
    fn build_glob_with_class_tokens() {
        Glob::new("a/[xy]").unwrap();
        Glob::new("a/[x-z]").unwrap();
        Glob::new("a/[xyi-k]").unwrap();
        Glob::new("a/[i-kxy]").unwrap();
        Glob::new("a/[!xy]").unwrap();
        Glob::new("a/[!x-z]").unwrap();
        Glob::new("a/[xy]b/c").unwrap();
    }

    #[test]
    fn build_glob_with_alternative_tokens() {
        Glob::new("a/{x?z,y$}b*").unwrap();
        Glob::new("a/{???,x$y,frob}b*").unwrap();
        Glob::new("a/{???,x$y,frob}b*").unwrap();
        Glob::new("a/{???,{x*z,y$}}b*").unwrap();
        Glob::new("a/{**/b,b/**}/ca{t,b/**}").unwrap();
    }

    #[test]
    fn build_glob_with_literal_escaped_wildcard_tokens() {
        Glob::new("a/b\\?/c").unwrap();
        Glob::new("a/b\\$/c").unwrap();
        Glob::new("a/b\\*/c").unwrap();
        Glob::new("a/b\\*\\*/c").unwrap();
    }

    #[test]
    fn build_glob_with_class_escaped_wildcard_tokens() {
        Glob::new("a/b[?]/c").unwrap();
        Glob::new("a/b[$]/c").unwrap();
        Glob::new("a/b[*]/c").unwrap();
        Glob::new("a/b[*][*]/c").unwrap();
    }

    #[test]
    fn build_glob_with_literal_escaped_alternative_tokens() {
        Glob::new("a/\\{\\}/c").unwrap();
        Glob::new("a/{x,y\\,,z}/c").unwrap();
    }

    #[test]
    fn build_glob_with_class_escaped_alternative_tokens() {
        Glob::new("a/[{][}]/c").unwrap();
        Glob::new("a/{x,y[,],z}/c").unwrap();
    }

    #[test]
    fn build_glob_with_literal_escaped_class_tokens() {
        Glob::new("a/\\[a-z\\]/c").unwrap();
        Glob::new("a/[\\[]/c").unwrap();
        Glob::new("a/[\\]]/c").unwrap();
        Glob::new("a/[a\\-z]/c").unwrap();
    }

    #[test]
    fn reject_glob_with_adjacent_tree_or_zom_tokens() {
        assert!(Glob::new("***").is_err());
        assert!(Glob::new("****").is_err());
        assert!(Glob::new("**/*/***").is_err());
        assert!(Glob::new("**$").is_err());
        assert!(Glob::new("**/$**").is_err());
    }

    #[test]
    fn reject_glob_with_tree_adjacent_literal_tokens() {
        assert!(Glob::new("**a").is_err());
        assert!(Glob::new("a**").is_err());
        assert!(Glob::new("a**b").is_err());
        assert!(Glob::new("a*b**").is_err());
        assert!(Glob::new("**/**a/**").is_err());
    }

    #[test]
    fn reject_glob_with_adjacent_one_tokens() {
        assert!(Glob::new("**?").is_err());
        assert!(Glob::new("?**").is_err());
        assert!(Glob::new("?**?").is_err());
        assert!(Glob::new("?*?**").is_err());
        assert!(Glob::new("**/**?/**").is_err());
    }

    #[test]
    fn reject_glob_with_unescaped_meta_characters_in_class_tokens() {
        assert!(Glob::new("a/[a-z-]/c").is_err());
        assert!(Glob::new("a/[-a-z]/c").is_err());
        assert!(Glob::new("a/[-]/c").is_err());
        // NOTE: Without special attention to escaping and character parsing,
        //       this could be mistakenly interpreted as an empty range over the
        //       character `-`. This should be rejected.
        assert!(Glob::new("a/[---]/c").is_err());
        assert!(Glob::new("a/[[]/c").is_err());
        assert!(Glob::new("a/[]]/c").is_err());
    }

    #[test]
    fn reject_glob_with_invalid_alternative_zom_tokens() {
        assert!(Glob::new("*{okay,*}").is_err());
        assert!(Glob::new("{okay,*}*").is_err());
        assert!(Glob::new("${okay,*error}").is_err());
        assert!(Glob::new("{okay,error*}$").is_err());
    }

    #[test]
    fn reject_glob_with_invalid_alternative_tree_tokens() {
        assert!(Glob::new("{**}").is_err());
        assert!(Glob::new("prefix{okay/**,**/error}").is_err());
        assert!(Glob::new("{**/okay,error/**}postfix").is_err());
        assert!(Glob::new("{**/okay,prefix{error/**}}postfix").is_err());
        assert!(Glob::new("{**/okay,prefix{**/error}}postfix").is_err());
    }

    #[test]
    fn reject_glob_with_invalid_separator_tokens() {
        assert!(Glob::new("//a").is_err());
        assert!(Glob::new("a//b").is_err());
        assert!(Glob::new("a/b//").is_err());
    }

    #[test]
    fn match_glob_with_tree_tokens() {
        let glob = Glob::new("a/**/b").unwrap();

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
                .unwrap(),
        );
    }

    #[test]
    fn match_glob_with_tree_and_zom_tokens() {
        let glob = Glob::new("**/*.ext").unwrap();

        assert!(glob.is_match(Path::new("file.ext")));
        assert!(glob.is_match(Path::new("a/file.ext")));
        assert!(glob.is_match(Path::new("a/b/file.ext")));

        let path = BytePath::from_path(Path::new("a/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"a/", captures.get(1).unwrap());
        assert_eq!(b"file", captures.get(2).unwrap());
    }

    #[test]
    fn match_glob_with_eager_and_lazy_zom_tokens() {
        let glob = Glob::new("$-*.*").unwrap();

        assert!(glob.is_match(Path::new("prefix-file.ext")));
        assert!(glob.is_match(Path::new("a-b-c.ext")));

        let path = BytePath::from_path(Path::new("a-b-c.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"a", captures.get(1).unwrap());
        assert_eq!(b"b-c", captures.get(2).unwrap());
        assert_eq!(b"ext", captures.get(3).unwrap());
    }

    #[test]
    fn match_glob_with_class_tokens() {
        let glob = Glob::new("a/[xyi-k]/**").unwrap();

        assert!(glob.is_match(Path::new("a/x/file.ext")));
        assert!(glob.is_match(Path::new("a/y/file.ext")));
        assert!(glob.is_match(Path::new("a/j/file.ext")));

        assert!(!glob.is_match(Path::new("a/b/file.ext")));

        let path = BytePath::from_path(Path::new("a/i/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"i", captures.get(1).unwrap());
    }

    #[test]
    fn match_glob_with_literal_escaped_class_tokens() {
        let glob = Glob::new("a/[\\[\\]\\-]/**").unwrap();

        assert!(glob.is_match(Path::new("a/[/file.ext")));
        assert!(glob.is_match(Path::new("a/]/file.ext")));
        assert!(glob.is_match(Path::new("a/-/file.ext")));

        assert!(!glob.is_match(Path::new("a/b/file.ext")));

        let path = BytePath::from_path(Path::new("a/[/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"[", captures.get(1).unwrap());
    }

    #[test]
    fn match_glob_with_alternative_tokens() {
        let glob = Glob::new("a/{x?z,y$}b/*").unwrap();

        assert!(glob.is_match(Path::new("a/xyzb/file.ext")));
        assert!(glob.is_match(Path::new("a/yb/file.ext")));

        assert!(!glob.is_match(Path::new("a/xyz/file.ext")));
        assert!(!glob.is_match(Path::new("a/y/file.ext")));
        assert!(!glob.is_match(Path::new("a/xyzub/file.ext")));

        let path = BytePath::from_path(Path::new("a/xyzb/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"xyz", captures.get(1).unwrap());
    }

    #[test]
    fn match_glob_with_nested_alternative_tokens() {
        let glob = Glob::new("a/{y$,{x?z,?z}}b/*").unwrap();

        let path = BytePath::from_path(Path::new("a/xyzb/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"xyz", captures.get(1).unwrap());
    }

    #[test]
    fn match_glob_with_alternative_tree_tokens() {
        let glob = Glob::new("a/{foo,bar,**/baz}/qux").unwrap();

        assert!(glob.is_match(Path::new("a/foo/qux")));
        assert!(glob.is_match(Path::new("a/foo/baz/qux")));
        assert!(glob.is_match(Path::new("a/foo/bar/baz/qux")));

        assert!(!glob.is_match(Path::new("a/foo/bar/qux")));
    }

    #[test]
    fn partition_glob_with_literal_and_non_literal_parts() {
        let (prefix, glob) = Glob::partitioned("a/b/x?z/*.ext").unwrap();

        assert_eq!(prefix, Path::new("a/b"));

        assert!(glob.is_match(Path::new("xyz/file.ext")));
        assert!(glob.is_match(Path::new("a/b/xyz/file.ext").strip_prefix(prefix).unwrap()));
    }

    #[test]
    fn partition_glob_with_only_non_literal_parts() {
        let (prefix, glob) = Glob::partitioned("x?z/*.ext").unwrap();

        assert_eq!(prefix, Path::new(""));

        assert!(glob.is_match(Path::new("xyz/file.ext")));
        assert!(glob.is_match(Path::new("xyz/file.ext").strip_prefix(prefix).unwrap()));
    }

    #[test]
    fn partition_glob_with_only_literal_parts() {
        let (prefix, glob) = Glob::partitioned("a/b").unwrap();

        assert_eq!(prefix, Path::new("a/b"));

        assert!(glob.is_match(Path::new("")));
        assert!(glob.is_match(Path::new("a/b").strip_prefix(prefix).unwrap()));
    }

    #[test]
    fn partition_glob_with_literal_dots_and_tree_tokens() {
        let (prefix, glob) = Glob::partitioned("../**/*.ext").unwrap();

        assert_eq!(prefix, Path::new(".."));

        assert!(glob.is_match(Path::new("xyz/file.ext")));
        assert!(glob.is_match(Path::new("../xyz/file.ext").strip_prefix(prefix).unwrap()));
    }
}
