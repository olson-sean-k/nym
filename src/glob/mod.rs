mod capture;
mod token;

use bstr::ByteVec;
use itertools::{EitherOrBoth, Itertools as _};
use nom::error::ErrorKind;
use os_str_bytes::OsStrBytes as _;
use regex::bytes::Regex;
use smallvec::SmallVec;
use std::borrow::{Borrow, Cow};
use std::ffi::OsStr;
use std::fs::FileType;
use std::path::{Component, Path, PathBuf, MAIN_SEPARATOR};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{self, DirEntry, WalkDir};

use crate::glob::token::{Token, Wildcard};
use crate::PositionExt as _;

pub use crate::glob::capture::Captures;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum GlobError {
    #[error("failed to parse glob: {0}")]
    Parse(nom::Err<(String, ErrorKind)>),
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
        Path::from_bytes(self.path.as_ref()).ok()
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
                    Grouping::NonCapture => pattern.push_str("(:?"),
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
            use crate::glob::token::Token::{
                Alternative, Class, Literal, NonTreeSeparator, Wildcard,
            };
            use crate::glob::token::Wildcard::{One, Tree, ZeroOrMore};

            for token in tokens.into_iter().with_position() {
                match token.interior_borrow().lift() {
                    (_, Literal(ref literal)) => {
                        for &byte in literal.as_bytes() {
                            pattern.push_str(&escape(byte));
                        }
                    }
                    (_, NonTreeSeparator) => pattern.push_str(&escape(b'/')),
                    (
                        _,
                        Alternative {
                            is_negated,
                            alternatives,
                        },
                    ) => {
                        let encodings: Vec<_> = alternatives
                            .iter()
                            .map(|tokens| {
                                let mut pattern = String::new();
                                pattern.push_str("(?:");
                                encode(Grouping::NonCapture, &mut pattern, tokens.iter());
                                pattern.push(')');
                                pattern
                            })
                            .collect();
                        if *is_negated {
                            // TODO: Migrate to `fancy-regex` and use look-ahead to encode a
                            //       negated group.
                            todo!(
                                "negated alternatives (zero-or-more-except) require features not \
                                 available in Nym's regular expression engine and are not yet \
                                 available"
                            );
                        }
                        else {
                            grouping.push_str(pattern, &encodings.join("|"));
                        }
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

    pub fn parse(text: &'t str) -> Result<Self, GlobError> {
        let tokens: Vec<_> = token::optimize(token::parse(text)?).collect();
        let regex = Glob::compile(tokens.iter());
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
        self.literal_path_prefix()
            .map(|prefix| prefix.is_absolute())
            .unwrap_or(false)
    }

    pub fn has_root(&self) -> bool {
        self.literal_path_prefix()
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
        let (prefix, root) = if let Some(prefix) = self.literal_path_prefix() {
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

    fn literal_path_prefix(&self) -> Option<PathBuf> {
        #[derive(Clone, Debug)]
        enum Component<'t> {
            Separator,
            Nominal(SmallVec<[&'t Token<'t>; 4]>),
        }

        let mut prefix = String::new();
        for component in
            self.tokens.iter().batching(|tokens| {
                let first = tokens.next();
                first.map(|first| {
                    if matches!(first, Token::NonTreeSeparator) {
                        Component::Separator
                    }
                    else {
                        Component::Nominal(
                            Some(first)
                                .into_iter()
                                .chain(tokens.take_while_ref(|token| {
                                    !matches!(token, Token::NonTreeSeparator)
                                }))
                                .collect(),
                        )
                    }
                })
            })
        {
            match component {
                Component::Separator => prefix.push(MAIN_SEPARATOR),
                Component::Nominal(tokens) => {
                    // NOTE: Tokens are optimized such that literals are
                    //       coalesced. These iterations typically operate on a
                    //       very small number of tokens.
                    if tokens
                        .iter()
                        .any(|token| !matches!(token, Token::Literal(_)))
                    {
                        // Abandon this component and construct the prefix if
                        // non-literal tokens are present.
                        break;
                    }
                    for token in tokens {
                        match *token {
                            Token::Literal(ref literal) => prefix.push_str(literal.as_ref()),
                            // See above; no non-literal tokens should be
                            // present here.
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
        if prefix.is_empty() {
            None
        }
        else {
            Some(prefix.into())
        }
    }
}

impl FromStr for Glob<'static> {
    type Err = GlobError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Glob::parse(text).map(|glob| glob.into_owned())
    }
}

struct Read<'g, 't> {
    glob: &'g Glob<'t>,
    regexes: Vec<Regex>,
    prefix: PathBuf,
    walk: walkdir::IntoIter,
}

impl<'g, 't> Read<'g, 't> {
    fn compile<I, T>(tokens: I) -> Vec<Regex>
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
                    })));
                }
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

    use crate::glob::{BytePath, Glob};

    #[test]
    fn parse_glob_with_eager_zom_tokens() {
        Glob::parse("*").unwrap();
        Glob::parse("a/*").unwrap();
        Glob::parse("*a").unwrap();
        Glob::parse("a*").unwrap();
        Glob::parse("a*b").unwrap();
        Glob::parse("/*").unwrap();
    }

    #[test]
    fn parse_glob_with_lazy_zom_tokens() {
        Glob::parse("$").unwrap();
        Glob::parse("a/$").unwrap();
        Glob::parse("$a").unwrap();
        Glob::parse("a$").unwrap();
        Glob::parse("a$b").unwrap();
        Glob::parse("/$").unwrap();
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
        Glob::parse("?$").unwrap();
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
    fn parse_glob_with_class_tokens() {
        Glob::parse("a/[xy]").unwrap();
        Glob::parse("a/[x-z]").unwrap();
        Glob::parse("a/[xyi-k]").unwrap();
        Glob::parse("a/[i-kxy]").unwrap();
        Glob::parse("a/[!xy]").unwrap();
        Glob::parse("a/[!x-z]").unwrap();
        Glob::parse("a/[xy]b/c").unwrap();
    }

    #[test]
    fn parse_glob_with_alternative_tokens() {
        Glob::parse("a/{x?z,y$}b*").unwrap();
        Glob::parse("a/{???,x$y,frob}b*").unwrap();
        Glob::parse("a/{???,x$y,frob}b*").unwrap();
        Glob::parse("a/{???,{x*z,y$}}b*").unwrap();
    }

    #[test]
    fn parse_glob_with_literal_escaped_wildcard_tokens() {
        Glob::parse("a/b\\?/c").unwrap();
        Glob::parse("a/b\\$/c").unwrap();
        Glob::parse("a/b\\*/c").unwrap();
        Glob::parse("a/b\\*\\*/c").unwrap();
    }

    #[test]
    fn parse_glob_with_class_escaped_wildcard_tokens() {
        Glob::parse("a/b[?]/c").unwrap();
        Glob::parse("a/b[$]/c").unwrap();
        Glob::parse("a/b[*]/c").unwrap();
        Glob::parse("a/b[*][*]/c").unwrap();
    }

    #[test]
    fn parse_glob_with_literal_escaped_alternative_tokens() {
        Glob::parse("a/\\{\\}/c").unwrap();
        Glob::parse("a/{x,y\\,,z}/c").unwrap();
    }

    #[test]
    fn parse_glob_with_class_escaped_alternative_tokens() {
        Glob::parse("a/[{][}]/c").unwrap();
        Glob::parse("a/{x,y[,],z}/c").unwrap();
    }

    #[test]
    fn parse_glob_with_literal_escaped_class_tokens() {
        Glob::parse("a/\\[a-z\\]/c").unwrap();
        Glob::parse("a/[\\[]/c").unwrap();
        Glob::parse("a/[\\]]/c").unwrap();
        Glob::parse("a/[a\\-z]/c").unwrap();
    }

    #[test]
    fn reject_glob_with_adjacent_tree_or_zom_tokens() {
        assert!(Glob::parse("***").is_err());
        assert!(Glob::parse("****").is_err());
        assert!(Glob::parse("**/*/***").is_err());
        assert!(Glob::parse("**$").is_err());
        assert!(Glob::parse("**/$**").is_err());
    }

    #[test]
    fn reject_glob_with_tree_adjacent_literal_tokens() {
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
    fn reject_glob_with_unescaped_meta_characters_in_class_tokens() {
        assert!(Glob::parse("a/[a-z-]/c").is_err());
        assert!(Glob::parse("a/[-a-z]/c").is_err());
        assert!(Glob::parse("a/[-]/c").is_err());
        // NOTE: Without special attention to escaping and character parsing,
        //       this could be mistakenly interpreted as an empty range over the
        //       character `-`. This should be rejected.
        assert!(Glob::parse("a/[---]/c").is_err());
        assert!(Glob::parse("a/[[]/c").is_err());
        assert!(Glob::parse("a/[]]/c").is_err());
    }

    #[test]
    fn literal_path_prefix() {
        assert_eq!(
            Glob::parse("a/b").unwrap().literal_path_prefix(),
            Some(Path::new("a/b").to_path_buf()),
        );
        assert_eq!(
            Glob::parse("a/*").unwrap().literal_path_prefix(),
            Some(Path::new("a/").to_path_buf()),
        );
        assert_eq!(
            Glob::parse("a/*b").unwrap().literal_path_prefix(),
            Some(Path::new("a/").to_path_buf()),
        );
        assert_eq!(
            Glob::parse("a/b*").unwrap().literal_path_prefix(),
            Some(Path::new("a/").to_path_buf()),
        );
        assert_eq!(
            Glob::parse("a/b/*/c").unwrap().literal_path_prefix(),
            Some(Path::new("a/b/").to_path_buf()),
        );

        assert!(Glob::parse("**").unwrap().literal_path_prefix().is_none());
        assert!(Glob::parse("a*").unwrap().literal_path_prefix().is_none());
        assert!(Glob::parse("*/b").unwrap().literal_path_prefix().is_none());
        assert!(Glob::parse("a?/b").unwrap().literal_path_prefix().is_none());
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
                .unwrap(),
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
        assert_eq!(b"a/", captures.get(1).unwrap());
        assert_eq!(b"file", captures.get(2).unwrap());
    }

    #[test]
    fn match_glob_with_eager_and_lazy_zom_tokens() {
        let glob = Glob::parse("$-*.*").unwrap();

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
        let glob = Glob::parse("a/[xyi-k]/**").unwrap();

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
        let glob = Glob::parse("a/[\\[\\]\\-]/**").unwrap();

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
        let glob = Glob::parse("a/{x?z,y$}b/*").unwrap();

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
        let glob = Glob::parse("a/{y$,{x?z,?z}}b/*").unwrap();

        let path = BytePath::from_path(Path::new("a/xyzb/file.ext"));
        let captures = glob.captures(&path).unwrap();
        assert_eq!(b"xyz", captures.get(1).unwrap());
    }

    #[test]
    fn match_glob_with_alternative_tree_tokens() {
        let glob = Glob::parse("a/{foo,bar,**/baz}/qux").unwrap();

        assert!(glob.is_match(Path::new("a/foo/qux")));
        assert!(glob.is_match(Path::new("a/foo/baz/qux")));
        assert!(glob.is_match(Path::new("a/foo/bar/baz/qux")));

        assert!(!glob.is_match(Path::new("a/foo/bar/qux")));
    }
}
