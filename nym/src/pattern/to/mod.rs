mod token;

use chrono::offset::Local;
use chrono::DateTime;
use std::borrow::Cow;
use std::path::Path;
use std::str::{self, FromStr};

use crate::glob::Captures;
use crate::pattern::to::token::{
    Capture, Condition, Identifier, NonEmptyCase, PropertyFormat, Subject, Substitution,
    TextFormatter, Token,
};
use crate::pattern::PatternError;
use crate::text;

#[derive(Clone, Debug)]
pub struct ToPattern<'t> {
    tokens: Vec<Token<'t>>,
}

impl<'t> ToPattern<'t> {
    pub fn parse(text: &'t str) -> Result<Self, PatternError> {
        token::parse(text).map(|tokens| ToPattern { tokens })
    }

    pub fn into_owned(self) -> ToPattern<'static> {
        let ToPattern { tokens } = self;
        let tokens = tokens.into_iter().map(|token| token.into_owned()).collect();
        ToPattern { tokens }
    }

    pub fn resolve(
        &self,
        source: impl AsRef<Path>,
        captures: &Captures<'_>,
    ) -> Result<String, PatternError> {
        use std::fs;

        use crate::memoize::Memoized;
        use crate::pattern::to::token::Property;

        #[cfg(feature = "property-b3sum")]
        let mut b3sum =
            Memoized::from(|| fs::read(source.as_ref()).map(|data| blake3::hash(data.as_ref())));
        let mut ctime = Memoized::from(|| {
            fs::metadata(source.as_ref())
                .and_then(|metadata| metadata.created())
                .map(DateTime::<Local>::from)
        });
        #[cfg(feature = "property-md5sum")]
        let mut md5sum = Memoized::from(|| fs::read(source.as_ref()).map(md5::compute));
        let mut mtime = Memoized::from(|| {
            fs::metadata(source.as_ref())
                .and_then(|metadata| metadata.modified())
                .map(DateTime::<Local>::from)
        });
        let mut output = String::new();
        for token in &self.tokens {
            match *token {
                Token::Substitution(Substitution {
                    ref subject,
                    ref formatters,
                }) => {
                    let (text, condition) = match subject {
                        Subject::Capture(Capture {
                            ref identifier,
                            ref condition,
                        }) => {
                            let capture = match identifier {
                                Identifier::Index(ref index) => captures.get(*index),
                                // TODO: Get captures by name when using
                                //       from-patterns that support it.
                                Identifier::Name(_) => None,
                            }
                            // Do not include empty captures. Captures that do
                            // not participate in a match and empty match text
                            // are treated the same way: the condition operates
                            // on an empty string.
                            .filter(|bytes| !bytes.is_empty())
                            .map(|bytes| str::from_utf8(bytes).map_err(PatternError::Encoding));
                            let capture: Cow<_> = if let Some(capture) = capture {
                                capture?.into()
                            }
                            else {
                                "".into()
                            };
                            (capture, condition.as_ref())
                        }
                        Subject::Property(ref property) => (
                            match *property {
                                #[cfg(feature = "property-b3sum")]
                                Property::B3Sum(ref fmt) => {
                                    b3sum.get().map_err(PatternError::Property)?.fmt(fmt).into()
                                }
                                Property::CTime(ref fmt) => {
                                    ctime.get().map_err(PatternError::Property)?.fmt(fmt).into()
                                }
                                #[cfg(feature = "property-md5sum")]
                                Property::Md5Sum(ref fmt) => md5sum
                                    .get()
                                    .map_err(PatternError::Property)?
                                    .fmt(fmt)
                                    .into(),
                                Property::MTime(ref fmt) => {
                                    mtime.get().map_err(PatternError::Property)?.fmt(fmt).into()
                                }
                            },
                            None,
                        ),
                    };
                    output.push_str(substitute(text.as_ref(), condition, formatters).as_ref());
                }
                Token::Literal(ref text) => {
                    output.push_str(text);
                }
            }
        }
        Ok(output)
    }
}

impl FromStr for ToPattern<'static> {
    type Err = PatternError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        ToPattern::parse(text).map(|pattern| pattern.into_owned())
    }
}

fn substitute<'t>(
    text: &'t str,
    condition: Option<&Condition<'t>>,
    formatters: &[TextFormatter],
) -> Cow<'t, str> {
    let text: Cow<str> = if let Some(condition) = condition {
        match (text.is_empty(), &condition.non_empty, &condition.empty) {
            (true, _, Some(ref empty)) => empty.0.clone(),
            (false, Some(ref non_empty), _) => match non_empty {
                NonEmptyCase::Surround {
                    ref prefix,
                    ref postfix,
                } => format!("{}{}{}", prefix, text, postfix,).into(),
                NonEmptyCase::Literal(ref literal) => literal.clone(),
            },
            (true, _, None) | (false, None, _) => text.into(),
        }
    }
    else {
        text.into()
    };
    if formatters.is_empty() {
        text
    }
    else {
        let mut text = text.into_owned();
        for formatter in formatters {
            text = match *formatter {
                TextFormatter::Pad {
                    shim,
                    alignment,
                    width,
                } => text::pad(&text, shim, alignment, width).into_owned(),
                TextFormatter::Lower => text.to_lowercase(),
                TextFormatter::Upper => text.to_uppercase(),
            };
        }
        text.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::pattern::ToPattern;

    #[test]
    fn parse_to_pattern() {
        ToPattern::parse("{}").unwrap();
        ToPattern::parse("{#1}").unwrap();
        ToPattern::parse("literal{#1}").unwrap();
        ToPattern::parse("{#1}literal").unwrap();
    }

    #[test]
    fn parse_to_pattern_condition() {
        ToPattern::parse("{#1?:}").unwrap();
        ToPattern::parse("{#1?[some]:}").unwrap();
        ToPattern::parse("{#1?[]:}").unwrap();
        ToPattern::parse("{#1?[prefix],[postfix]:}").unwrap();
        ToPattern::parse("{#1?:[none]}").unwrap();
        ToPattern::parse("{#1?[],[-]:[none]}").unwrap();
    }

    #[test]
    fn parse_to_pattern_formatter() {
        ToPattern::parse("{#1|>4[0]}").unwrap();
        ToPattern::parse("{#1|u}").unwrap();
        ToPattern::parse("{#1|<2[ ],l}").unwrap();
    }

    #[test]
    fn parse_to_pattern_condition_formatter() {
        ToPattern::parse("{#1?[prefix],[postfix]:[none]|>4[0]}").unwrap();
    }

    #[test]
    fn parse_to_pattern_with_escaped_literal() {
        ToPattern::parse("a/b/file\\{0\\}.ext").unwrap();
        ToPattern::parse("a/b/file\\[0\\].ext").unwrap();
        // NOTE: Escaping square brackets is not necessary in literals.
        ToPattern::parse("a/b/file[0].ext").unwrap();
    }

    #[test]
    fn parse_to_pattern_with_escaped_argument() {
        ToPattern::parse("{#1?[\\[\\]]:}").unwrap();
        // NOTE: Escaping curly braces is not necessary in arguments.
        ToPattern::parse("{#1?[{}]:[\\{\\}]}").unwrap();
        ToPattern::parse("{@[capture\\[0\\]]}").unwrap();
    }

    #[test]
    fn reject_to_pattern_with_empty_case_surround() {
        assert!(ToPattern::parse("{#1?:[prefix],[postfix]}").is_err());
    }

    #[test]
    fn reject_to_pattern_out_of_order() {
        assert!(ToPattern::parse("{#1|u?:}").is_err());
    }
}
