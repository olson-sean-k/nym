use bimap::BiMap;
use console::{Style, Term};
use itertools::{Itertools as _, Position};
use std::borrow::Borrow;
use std::cmp;
use std::io::prelude::*;
use std::io::{self, Error, ErrorKind};
use std::iter;
use std::path::PathBuf;
use textwrap;

pub trait Manifest: Default + IntoIterator<Item = (PathBuf, PathBuf)> {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()>;

    fn print(&self, terminal: &mut Term) -> io::Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<PathBuf, PathBuf>,
}

impl IntoIterator for Bijective {
    type Item = <BiMap<PathBuf, PathBuf> as IntoIterator>::Item;
    type IntoIter = <BiMap<PathBuf, PathBuf> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl Manifest for Bijective {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()> {
        self.inner
            .insert_no_overwrite(source.into(), destination.into())
            .map_err(|_| Error::from(ErrorKind::Other))
    }

    fn print(&self, terminal: &mut Term) -> io::Result<()> {
        print(
            terminal,
            self.inner
                .iter()
                .map(|(source, terminal)| (Some(source), terminal)),
        )
    }
}

fn print<P, I>(terminal: &mut Term, paths: impl ExactSizeIterator<Item = (I, P)>) -> io::Result<()>
where
    P: Borrow<PathBuf>,
    I: IntoIterator<Item = P>,
{
    const MIN_TERMINAL_WIDTH: usize = 16;

    let margin = ((paths.len() as f64).log10() as usize) + 1;
    let width = terminal.size().1 as usize;
    let width = cmp::max(width - cmp::min(width, margin + 6), MIN_TERMINAL_WIDTH);
    for (n, (sources, destination)) in paths.enumerate() {
        for source in sources.into_iter().with_position() {
            match source {
                Position::First(source) | Position::Only(source) => {
                    let source = source.borrow().to_string_lossy();
                    for line in textwrap::wrap(source.as_ref(), width)
                        .into_iter()
                        .with_position()
                    {
                        match line {
                            Position::First(line) | Position::Only(line) => write!(
                                terminal,
                                "{:0>width$} ─┬── {}\n",
                                Style::new().bright().white().apply_to(n),
                                Style::new().green().apply_to(line),
                                width = margin,
                            ),
                            Position::Middle(line) | Position::Last(line) => write!(
                                terminal,
                                "{: >width$}   {}\n",
                                "│",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                        }?;
                    }
                }
                Position::Middle(source) | Position::Last(source) => {
                    let source = source.borrow().to_string_lossy();
                    for line in textwrap::wrap(source.as_ref(), width)
                        .into_iter()
                        .with_position()
                    {
                        match line {
                            Position::First(line) | Position::Only(line) => write!(
                                terminal,
                                "{: >width$} {}\n",
                                "├──",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                            Position::Middle(line) | Position::Last(line) => write!(
                                terminal,
                                "{: >width$}   {}\n",
                                "│",
                                Style::new().green().apply_to(line),
                                width = margin + 3,
                            ),
                        }?;
                    }
                }
            }
        }
        let destination = destination.borrow().to_string_lossy();
        for line in textwrap::wrap(destination.as_ref(), width)
            .into_iter()
            .with_position()
        {
            match line {
                Position::First(line) | Position::Only(line) => write!(
                    terminal,
                    "{: >width$} {}\n",
                    "╰─⯈",
                    Style::new().bold().red().apply_to(line),
                    width = margin + 5,
                ),
                Position::Middle(line) | Position::Last(line) => write!(
                    terminal,
                    "{: >width$}{}\n",
                    "",
                    Style::new().bold().red().apply_to(line),
                    width = margin + 6,
                ),
            }?;
        }
    }
    Ok(())
}
