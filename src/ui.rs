use console::{Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressIterator};
use itertools::{Itertools as _, Position};
use std::cmp;
use std::io;
use std::io::prelude::*;
use std::path::Path;

const MIN_TERMINAL_WIDTH: usize = 16;

pub trait IteratorExt: Iterator + Sized {
    fn print_actuator_progress(self, terminal: Term) -> ProgressBarIter<Self>
    where
        Self: ExactSizeIterator,
    {
        let n = self.len() as u64;
        let bar = ProgressBar::with_draw_target(n, ProgressDrawTarget::to_term(terminal, 15));
        self.progress_with(bar)
    }
}

impl<I> IteratorExt for I where I: Iterator + Sized {}

pub fn confirmation(terminal: &Term, prompt: impl AsRef<str>) -> io::Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt.as_ref())
        .default(false)
        .show_default(true)
        .wait_for_newline(true)
        .interact_on(terminal)
}

pub fn print_grouped_paths<P, I>(terminal: &mut Term, paths: &[(I, P)]) -> io::Result<()>
where
    P: AsRef<Path>,
    I: Clone + IntoIterator<Item = P>,
{
    let margin = ((paths.len() as f64).log10() as usize) + 1;
    let width = terminal.size().1 as usize;
    let width = cmp::max(width - cmp::min(width, margin + 6), MIN_TERMINAL_WIDTH);
    for (n, (sources, destination)) in paths.iter().enumerate() {
        for source in sources.clone().into_iter().with_position() {
            match source {
                Position::First(source) | Position::Only(source) => {
                    let source = source.as_ref().to_string_lossy();
                    for line in textwrap::wrap(source.as_ref(), width)
                        .into_iter()
                        .with_position()
                    {
                        match line {
                            Position::First(line) | Position::Only(line) => writeln!(
                                terminal,
                                "{:0>width$} ─┬── {}",
                                Style::new().bright().white().apply_to(n + 1),
                                Style::new().green().apply_to(line),
                                width = margin,
                            ),
                            Position::Middle(line) | Position::Last(line) => writeln!(
                                terminal,
                                "{: >width$}   {}",
                                "│",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                        }?;
                    }
                }
                Position::Middle(source) | Position::Last(source) => {
                    let source = source.as_ref().to_string_lossy();
                    for line in textwrap::wrap(source.as_ref(), width)
                        .into_iter()
                        .with_position()
                    {
                        match line {
                            Position::First(line) | Position::Only(line) => writeln!(
                                terminal,
                                "{: >width$} {}",
                                "├──",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                            Position::Middle(line) | Position::Last(line) => writeln!(
                                terminal,
                                "{: >width$}   {}",
                                "│",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                        }?;
                    }
                }
            }
        }
        let destination = destination.as_ref().to_string_lossy();
        for line in textwrap::wrap(destination.as_ref(), width)
            .into_iter()
            .with_position()
        {
            match line {
                Position::First(line) | Position::Only(line) => writeln!(
                    terminal,
                    "{: >width$} {}",
                    "╰─⯈",
                    Style::new().bold().red().apply_to(line),
                    width = margin + 5,
                ),
                Position::Middle(line) | Position::Last(line) => writeln!(
                    terminal,
                    "{: >width$}{}",
                    "",
                    Style::new().bold().red().apply_to(line),
                    width = margin + 6,
                ),
            }?;
        }
    }
    Ok(())
}
