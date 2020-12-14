use console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressIterator};
use itertools::{Itertools as _, Position};
use lazy_static::lazy_static;
use std::cmp;
use std::io;
use std::io::prelude::*;

use nym::actuator::{Copy, Move};
use nym::manifest::{Manifest, Routing};

const MIN_TERMINAL_WIDTH: usize = 16;

lazy_static! {
    static ref STYLE_BOX: Style = Style::new().white();
    static ref STYLE_BRIGHT: Style = Style::new().bright().white();
    static ref STYLE_GREEN: Style = Style::new().green();
    static ref STYLE_RED: Style = Style::new().bold().red();
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

impl Label for Move {
    const LABEL: &'static str = "move";
}

pub trait Print {
    fn print(&self, terminal: &mut Term) -> io::Result<()>;
}

impl<M> Print for Manifest<M>
where
    M: Routing,
{
    fn print(&self, terminal: &mut Term) -> io::Result<()> {
        let paths = self.paths();
        let margin = ((paths.len() as f64).log10() as usize) + 1;
        let width = terminal.size().1 as usize;
        let width = cmp::max(width - cmp::min(width, margin + 6), MIN_TERMINAL_WIDTH);
        for (n, (sources, destination)) in paths.enumerate() {
            for source in sources.clone().into_iter().with_position() {
                match source {
                    Position::First(source) | Position::Only(source) => {
                        let source = source.to_string_lossy();
                        for line in textwrap::wrap(source.as_ref(), width)
                            .into_iter()
                            .with_position()
                        {
                            match line {
                                Position::First(line) | Position::Only(line) => writeln!(
                                    terminal,
                                    "{:0>width$} {} {}",
                                    STYLE_BRIGHT.apply_to(n + 1),
                                    STYLE_BOX.apply_to("─┬──"),
                                    STYLE_GREEN.apply_to(line),
                                    width = margin,
                                ),
                                Position::Middle(line) | Position::Last(line) => writeln!(
                                    terminal,
                                    "{: >width$}   {}",
                                    STYLE_BOX.apply_to("│"),
                                    STYLE_GREEN.apply_to(line),
                                    width = margin + 3,
                                ),
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
                                Position::First(line) | Position::Only(line) => writeln!(
                                    terminal,
                                    "{: >width$} {}",
                                    STYLE_BOX.apply_to("├──"),
                                    STYLE_GREEN.apply_to(line),
                                    width = margin + 3,
                                ),
                                Position::Middle(line) | Position::Last(line) => writeln!(
                                    terminal,
                                    "{: >width$}   {}",
                                    STYLE_BOX.apply_to("│"),
                                    STYLE_GREEN.apply_to(line),
                                    width = margin + 3,
                                ),
                            }?;
                        }
                    }
                }
            }
            let destination = destination.to_string_lossy();
            for line in textwrap::wrap(destination.as_ref(), width)
                .into_iter()
                .with_position()
            {
                match line {
                    Position::First(line) | Position::Only(line) => writeln!(
                        terminal,
                        "{: >width$} {}",
                        STYLE_BOX.apply_to("╰─⯈"),
                        STYLE_RED.apply_to(line),
                        width = margin + 5,
                    ),
                    Position::Middle(line) | Position::Last(line) => writeln!(
                        terminal,
                        "{: >width$}{}",
                        "",
                        STYLE_RED.apply_to(line),
                        width = margin + 6,
                    ),
                }?;
            }
        }
        Ok(())
    }
}

pub fn confirmation(terminal: &Term, prompt: impl AsRef<str>) -> io::Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt.as_ref())
        .default(false)
        .show_default(true)
        .wait_for_newline(true)
        .interact_on(terminal)
}
