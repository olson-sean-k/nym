mod ui;

use anyhow::Error;
use console::Term;
use regex::bytes::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Copy, Move, Operation};
use nym::environment::{Environment, Policy};
use nym::glob::Glob;
use nym::manifest::Manifest;
use nym::pattern::{FromPattern, ToPattern};

use crate::ui::{IteratorExt as _, Label, Print};

const DISCLAIMER: &str = "paths may be ambiguous and undetected collisions may cause overwriting, \
                          truncation, and data loss; review patterns and paths carefully.";

/// Append, copy, link, and move files using patterns.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    /// Use regular expressions (instead of globs) for from-patterns.
    #[structopt(long = "regex", short = "X")]
    regex: bool,
    /// The working directory tree.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
    /// Descend at most to this depth in the working directory tree.
    ///
    /// A depth of zero only includes files within the working directory (there
    /// is no recursion).
    #[structopt(long = "depth", default_value = "1000000")]
    depth: usize,
    /// Perform operations without interactive prompts and ignoring warnings.
    #[structopt(long = "force", short = "f")]
    force: bool,
    /// Overwrite existing files resolved by to-patterns.
    #[structopt(long = "overwrite", short = "w")]
    overwrite: bool,
    /// Create parent directories for paths resolved by to-patterns.
    #[structopt(long = "parents", short = "p")]
    parents: bool,
    /// Do not print additional information nor warnings.
    #[structopt(long = "quiet", short = "q")]
    quiet: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    /// Appends matched files.
    Append {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
    /// Copies matched files.
    Copy {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
    /// Symbolically links matched files.
    Link {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
    /// Moves matched files.
    Move {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
}

/// Transformation.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct UnparsedTransform {
    /// The from-pattern used to match source files.
    from: String,
    /// The to-pattern used to resolve destination files.
    to: String,
}

impl UnparsedTransform {
    fn parse(&self, is_regex: bool) -> Result<(FromPattern<'_>, ToPattern<'_>), Error> {
        let from = if is_regex {
            Regex::new(&self.from)?.into()
        }
        else {
            Glob::parse(&self.from)?.into()
        };
        let to = ToPattern::parse(&self.to)?;
        Ok((from, to))
    }
}

struct Harness {
    environment: Environment,
    directory: PathBuf,
    depth: usize,
    force: bool,
    quiet: bool,
}

impl Harness {
    fn run<A>(&self, from: FromPattern<'_>, to: ToPattern<'_>) -> Result<(), Error>
    where
        A: Label + Operation,
    {
        let terminal = Term::stderr();
        let transform = self.environment.transform(from, to);
        let actuator = self.environment.actuator();
        let manifest: Manifest<A::Routing> = transform.read(&self.directory, self.depth)?;
        if !self.quiet {
            manifest.print(&terminal)?;
            ui::print_warning(&terminal, DISCLAIMER)?;
        }
        if self.force
            || ui::confirmation(
                &terminal,
                format!(
                    "Ready to {} into {} files. Continue?",
                    A::LABEL,
                    manifest.routes().len(),
                ),
            )?
        {
            for route in manifest.routes().print_progress(terminal) {
                actuator.write::<A, _>(route)?;
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    let harness = Harness {
        environment: Environment::new(Policy {
            parents: options.parents,
            overwrite: options.overwrite,
        }),
        directory: options.directory,
        depth: options.depth + 1,
        force: options.force,
        quiet: options.quiet,
    };
    match options.command {
        Command::Append { .. } => todo!(),
        Command::Copy { transform, .. } => {
            let (from, to) = transform.parse(options.regex)?;
            harness.run::<Copy>(from, to)?;
        }
        Command::Link { .. } => todo!(),
        Command::Move { transform, .. } => {
            let (from, to) = transform.parse(options.regex)?;
            harness.run::<Move>(from, to)?;
        }
    }
    Ok(())
}
