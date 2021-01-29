use bstr::ByteVec;
use itertools::{Itertools as _, Position};
use nom::error::ErrorKind;
use regex::bytes::Regex;
use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;
use walkdir::{self, WalkDir};

use crate::PositionExt as _;

pub use regex::bytes::Captures;
use std::ffi::OsStr;

const GLOB_SEPARATOR: u8 = b'/';

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
pub struct BytePath<'a> {
    path: Cow<'a, [u8]>,
}

impl<'a> BytePath<'a> {
    fn from_bytes(bytes: Cow<'a, [u8]>) -> Self {
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

    pub fn from_os_str(text: &'a OsStr) -> Self {
        Self::from_bytes(Vec::from_os_str_lossy(text))
    }

    pub fn from_path<P>(path: &'a P) -> Self
    where
        P: AsRef<Path> + ?Sized,
    {
        Self::from_bytes(Vec::from_path_lossy(path.as_ref()))
    }
}

impl<'a> AsRef<[u8]> for BytePath<'a> {
    fn as_ref(&self) -> &[u8] {
        self.path.as_ref()
    }
}

#[derive(Clone, Copy, Debug)]
enum Wildcard {
    One,        // ?
    ZeroOrMore, // *
    Tree,       // **
}

#[derive(Clone, Debug)]
enum Token<'a> {
    Literal(Cow<'a, str>),
    NonTreeSeparator,
    Wildcard(Wildcard),
}

impl<'a> Token<'a> {
    pub fn into_owned(self) -> Token<'static> {
        match self {
            Token::Literal(literal) => literal.into_owned().into(),
            Token::NonTreeSeparator => Token::NonTreeSeparator,
            Token::Wildcard(wildcard) => Token::Wildcard(wildcard),
        }
    }
}

impl<'a> From<&'a str> for Token<'a> {
    fn from(literal: &'a str) -> Self {
        Token::Literal(literal.into())
    }
}

impl From<String> for Token<'static> {
    fn from(literal: String) -> Self {
        Token::Literal(literal.into())
    }
}

impl From<Wildcard> for Token<'static> {
    fn from(wildcard: Wildcard) -> Self {
        Token::Wildcard(wildcard)
    }
}

#[derive(Clone, Debug)]
pub struct Glob<'a> {
    tokens: Vec<Token<'a>>,
    regex: Regex,
}

impl<'a> Glob<'a> {
    fn from_tokens<I>(tokens: I) -> Result<Self, GlobError>
    where
        I: IntoIterator<Item = Token<'a>>,
    {
        const ASCII_TERMINATOR: u8 = 0x7F;
        let tokens: Vec<_> = tokens
            .into_iter()
            .dedup_by(|left, right| {
                matches!(
                    (left, right),
                    (
                        Token::Wildcard(Wildcard::Tree),
                        Token::Wildcard(Wildcard::Tree)
                    )
                )
            })
            .dedup_by(|left, right| {
                matches!(
                    (left, right),
                    (
                        Token::Wildcard(Wildcard::ZeroOrMore),
                        Token::Wildcard(Wildcard::ZeroOrMore)
                    )
                )
            })
            .filter(|token| match &token {
                Token::Literal(ref literal) => !literal.is_empty(),
                _ => true,
            })
            .coalesce(|left, right| match (&left, &right) {
                (Token::Literal(ref left), Token::Literal(ref right)) => {
                    Ok(Token::Literal(format!("{}{}", left, right).into()))
                }
                _ => Err((left, right)),
            })
            .collect();
        let mut pattern = String::new();
        let mut push = |text: &str| pattern.push_str(text);
        let escape = |byte: u8| {
            if byte <= ASCII_TERMINATOR {
                regex::escape(&(byte as char).to_string())
            }
            else {
                format!("\\x{:02x}", byte)
            }
        };
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

    pub fn parse(text: &'a str) -> Result<Self, GlobError> {
        use nom::bytes::complete as bytes;
        use nom::error::{FromExternalError, ParseError};
        use nom::{branch, combinator, multi, sequence, IResult, Parser};

        fn no_adjacent_tree<'i, O, E, F>(parser: F) -> impl FnMut(&'i str) -> IResult<&'i str, O, E>
        where
            E: ParseError<&'i str>,
            F: Parser<&'i str, O, E>,
        {
            sequence::delimited(
                combinator::peek(combinator::not(bytes::tag("**"))),
                parser,
                combinator::peek(combinator::not(bytes::tag("**"))),
            )
        }

        // TODO: Support escaping wildcards as literals.
        fn literal<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::map(no_adjacent_tree(bytes::is_not("/?*")), From::from)(input)
        }

        fn wildcard<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            branch::alt((
                combinator::map(no_adjacent_tree(bytes::tag("?")), |_| {
                    Token::from(Wildcard::One)
                }),
                combinator::map(
                    sequence::delimited(
                        branch::alt((bytes::tag("/"), bytes::tag(""))),
                        bytes::tag("**"),
                        branch::alt((bytes::tag("/"), combinator::eof)),
                    ),
                    |_| Token::from(Wildcard::Tree),
                ),
                combinator::map(
                    sequence::terminated(
                        bytes::tag("*"),
                        branch::alt((combinator::peek(bytes::is_not("*")), combinator::eof)),
                    ),
                    |_| Token::from(Wildcard::ZeroOrMore),
                ),
            ))(input)
        }

        fn separator<'i, E>(input: &'i str) -> IResult<&'i str, Token, E>
        where
            E: ParseError<&'i str>,
        {
            combinator::value(Token::NonTreeSeparator, bytes::tag("/"))(input)
        }

        fn glob<'i, E>(input: &'i str) -> IResult<&'i str, Glob, E>
        where
            E: FromExternalError<&'i str, GlobError> + ParseError<&'i str>,
        {
            combinator::all_consuming(combinator::map_res(
                multi::many1(branch::alt((wildcard, separator, literal))),
                Glob::from_tokens,
            ))(input)
        }

        glob::<(_, ErrorKind)>(text)
            .map(|(_, glob)| glob)
            .map_err(Into::into)
    }

    pub fn into_owned(self) -> Glob<'static> {
        // Taking ownership of token data does not modify the regular
        // expression.
        let Glob { tokens, regex } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        Glob { tokens, regex }
    }

    pub fn is_absolute(&self) -> bool {
        self.literal_prefix()
            .map(|literal| {
                let path = Path::new(literal.as_ref());
                path.is_absolute()
            })
            .unwrap_or(false)
    }

    pub fn has_root(&self) -> bool {
        self.literal_prefix()
            .map(|literal| {
                let path = Path::new(literal.as_ref());
                path.has_root()
            })
            .unwrap_or(false)
    }

    // TODO: Unix-like globs do not interact well with Windows path prefixes.
    pub fn has_prefix(&self) -> bool {
        self.literal_prefix()
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
        text: &'a str,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Read<'a>, GlobError> {
        let glob = Glob::parse(text)?;
        let directory = if let Some(prefix) = glob.directory_prefix() {
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

    fn literal_prefix(&self) -> Option<&Cow<'a, str>> {
        self.tokens.get(0).and_then(|token| match *token {
            Token::Literal(ref literal) => Some(literal),
            _ => None,
        })
    }

    fn directory_prefix(&self) -> Option<&Path> {
        self.literal_prefix().and_then(|literal| {
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

pub struct Read<'a> {
    glob: Glob<'a>,
    components: Vec<Glob<'a>>,
    //strip: PathBuf,
    walk: walkdir::IntoIter,
}

impl<'a> Read<'a> {
    pub fn glob(&self) -> &Glob<'a> {
        &self.glob
    }
}

impl<'a> Iterator for Read<'a> {
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
