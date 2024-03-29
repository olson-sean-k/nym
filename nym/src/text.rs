use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    Left,
    Right,
    Center,
}

pub fn coalesce(text: &str, from: &[char], to: char) -> String {
    text.chars()
        .map(|character| {
            if from.contains(&character) {
                to
            }
            else {
                character
            }
        })
        .collect()
}

pub fn pad(text: &str, shim: char, alignment: Alignment, width: usize) -> Cow<str> {
    let n = UnicodeWidthStr::width(text);
    if n >= width {
        text.into()
    }
    else {
        let margin = width - n;
        let (left, right) = match alignment {
            Alignment::Left => (0, margin),
            Alignment::Right => (margin, 0),
            Alignment::Center => (margin / 2, margin - (margin / 2)),
        };
        let mut padded = String::new();
        for _ in 0..left {
            padded.push(shim);
        }
        padded.push_str(text);
        for _ in 0..right {
            padded.push(shim);
        }
        padded.into()
    }
}

#[cfg(test)]
mod tests {
    use crate::text::{self, Alignment};

    #[test]
    fn coalesce_identity() {
        assert_eq!(
            text::coalesce("the quick brown fox", &[' '], ' '),
            "the quick brown fox"
        );
    }

    #[test]
    fn coalesce_one_to_one() {
        assert_eq!(
            text::coalesce("the quick brown fox", &[' '], '-'),
            "the-quick-brown-fox"
        );
    }

    #[test]
    fn coalesce_many_to_one() {
        assert_eq!(
            text::coalesce("the_quick-brown\tfox", &['_', '-', '\t'], ' '),
            "the quick brown fox"
        );
    }

    #[test]
    fn pad_left() {
        assert_eq!(
            text::pad("text", ' ', Alignment::Left, 8).as_ref(),
            "text    "
        );
    }

    #[test]
    fn pad_right() {
        assert_eq!(
            text::pad("text", ' ', Alignment::Right, 8).as_ref(),
            "    text"
        );
    }

    #[test]
    fn pad_center() {
        assert_eq!(
            text::pad("text", ' ', Alignment::Center, 8).as_ref(),
            "  text  "
        );
    }

    #[test]
    fn pad_left_overflow() {
        assert_eq!(
            text::pad("too much text", ' ', Alignment::Left, 8).as_ref(),
            "too much text"
        );
    }
}
