use std::borrow::Cow;
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    Left,
    Right,
    Center,
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
    use crate::fmt::{self, Alignment};

    #[test]
    fn pad_left() {
        assert_eq!(
            fmt::pad("text", ' ', Alignment::Left, 8).as_ref(),
            "text    "
        );
    }

    #[test]
    fn pad_right() {
        assert_eq!(
            fmt::pad("text", ' ', Alignment::Right, 8).as_ref(),
            "    text"
        );
    }

    #[test]
    fn pad_center() {
        assert_eq!(
            fmt::pad("text", ' ', Alignment::Center, 8).as_ref(),
            "  text  "
        );
    }

    #[test]
    fn pad_left_overflow() {
        assert_eq!(
            fmt::pad("too much text", ' ', Alignment::Left, 8).as_ref(),
            "too much text"
        );
    }
}
