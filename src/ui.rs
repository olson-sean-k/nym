use console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressIterator};
use itertools::{Itertools as _, Position};
use lazy_static::lazy_static;
use std::cmp;
use std::io;

use nym::actuator::{Copy, HardLink, Move, SoftLink};
use nym::manifest::{Manifest, Routing};

const MIN_TERMINAL_WIDTH: usize = 16;

lazy_static! {
    static ref STYLE_ARROW: Style = Style::new();
    static ref STYLE_INDEX: Style = Style::new().bright().white();
    static ref STYLE_SOURCE_PATH: Style = Style::new().green();
    static ref STYLE_DESTINATION_PATH: Style = Style::new().red();
    static ref STYLE_WARNING: Style = Style::new().bold();
    static ref STYLE_WARNING_HEADER: Style = Style::new().blink().bold().yellow();
}

pub trait IteratorExt: Iterator + Sized {
    fn print_progress(self, terminal: Term) -> ProgressBarIter<Self>
    where
        Self: ExactSizeIterator,
    {
        let n = self.len() as u64;
        let bar = ProgressBar::with_draw_target(n, ProgressDrawTarget::to_term(terminal, 15));
        self.progress_with(bar)
    }
}

impl<I> IteratorExt for I where I: Iterator + Sized {}

pub trait Label {
    const LABEL: &'static str;
}

impl Label for Copy {
    const LABEL: &'static str = "copy";
}

impl Label for HardLink {
    const LABEL: &'static str = "hard link";
}

impl Label for Move {
    const LABEL: &'static str = "move";
}

impl Label for SoftLink {
    const LABEL: &'static str = "soft link";
}

pub trait Print {
    fn print(&self, terminal: &Term) -> io::Result<()>;
}

impl<M> Print for Manifest<M>
where
    M: Routing,
{
    fn print(&self, terminal: &Term) -> io::Result<()> {
        let routes = self.routes();
        let margin = ((routes.len() as f64).log10() as usize) + 1;
        let width = width(terminal, margin + 6);
        for (n, route) in routes.enumerate() {
            for source in route.sources().with_position() {
                match source {
                    Position::First(source) | Position::Only(source) => {
                        let source = source.to_string_lossy();
                        for line in textwrap::wrap(source.as_ref(), width)
                            .into_iter()
                            .with_position()
                        {
                            match line {
                                Position::First(line) | Position::Only(line) => terminal
                                    .write_line(&format!(
                                        "{:0>width$} {} {}",
                                        STYLE_INDEX.apply_to(n + 1),
                                        STYLE_ARROW.apply_to("─┬──"),
                                        STYLE_SOURCE_PATH.apply_to(line),
                                        width = margin,
                                    )),
                                Position::Middle(line) | Position::Last(line) => terminal
                                    .write_line(&format!(
                                        "{: >width$}   {}",
                                        STYLE_ARROW.apply_to("│"),
                                        STYLE_SOURCE_PATH.apply_to(line),
                                        width = margin + 3,
                                    )),
                            }?;
                        }
                    }
                    Position::Middle(source) | Position::Last(source) => {
                        let source = source.to_string_lossy();
                        for line in textwrap::wrap(source.as_ref(), width)
                            .into_iter()
                            .with_position()
                        {
                            match line {
                                Position::First(line) | Position::Only(line) => terminal
                                    .write_line(&format!(
                                        "{: >width$} {}",
                                        STYLE_ARROW.apply_to("├──"),
                                        STYLE_SOURCE_PATH.apply_to(line),
                                        width = margin + 3,
                                    )),
                                Position::Middle(line) | Position::Last(line) => terminal
                                    .write_line(&format!(
                                        "{: >width$}   {}",
                                        STYLE_ARROW.apply_to("│"),
                                        STYLE_SOURCE_PATH.apply_to(line),
                                        width = margin + 3,
                                    )),
                            }?;
                        }
                    }
                }
            }
            let destination = route.destination().to_string_lossy();
            for line in textwrap::wrap(destination.as_ref(), width)
                .into_iter()
                .with_position()
            {
                match line {
                    Position::First(line) | Position::Only(line) => terminal.write_line(&format!(
                        "{: >width$} {}",
                        STYLE_ARROW.apply_to("╰─⯈"),
                        STYLE_DESTINATION_PATH.apply_to(line),
                        width = margin + 5,
                    )),
                    Position::Middle(line) | Position::Last(line) => terminal.write_line(&format!(
                        "{: >width$}{}",
                        "",
                        STYLE_DESTINATION_PATH.apply_to(line),
                        width = margin + 6,
                    )),
                }?;
            }
        }
        Ok(())
    }
}

pub fn print_warning(terminal: &Term, warning: impl AsRef<str>) -> io::Result<()> {
    const HEADER: &str = "Warning";
    let margin = HEADER.len() + 2;
    let width = width(terminal, margin);
    for line in textwrap::wrap(warning.as_ref(), width)
        .into_iter()
        .with_position()
    {
        match line {
            Position::First(line) | Position::Only(line) => terminal.write_line(&format!(
                "{}{} {}",
                STYLE_WARNING_HEADER.apply_to(HEADER),
                STYLE_WARNING.apply_to(":"),
                STYLE_WARNING.apply_to(line),
            )),
            Position::Middle(line) | Position::Last(line) => terminal.write_line(&format!(
                "{: <width$}{}",
                "",
                STYLE_WARNING.apply_to(line),
                width = margin,
            )),
        }?;
    }
    Ok(())
}

pub fn confirm(terminal: &Term, prompt: impl AsRef<str>) -> io::Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt.as_ref())
        .default(false)
        .show_default(true)
        .wait_for_newline(true)
        .interact_on(terminal)
}

fn width(terminal: &Term, margin: usize) -> usize {
    let width = terminal.size().1 as usize;
    cmp::max(width - cmp::min(width, margin), MIN_TERMINAL_WIDTH)
}
