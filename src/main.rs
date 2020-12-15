mod ui;

use anyhow::Error;
use console::Term;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Actuator, Copy, Move, Operation};
use nym::manifest::Manifest;
use nym::pattern::ToPattern;
use nym::transform::Transform;

use crate::ui::{IteratorExt as _, Label, Print};

/// Append, copy, and move files using patterns.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    /// Use path globs for from-patterns.
    #[structopt(long = "glob", short = "G", conflicts_with = "regex")]
    glob: bool,
    /// Use regular expressions for from-patterns.
    #[structopt(long = "regex", short = "X")]
    regex: bool,
    /// The working directory.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
    /// Perform operations without interactive prompts and ignoring warnings.
    #[structopt(long = "force", short = "f")]
    force: bool,
    /// Overwrite existing files matched by to-patterns.
    #[structopt(long = "overwrite", short = "w")]
    overwrite: bool,
    /// Create parent directories for paths matched by to-patterns.
    #[structopt(long = "parents", short = "p")]
    parents: bool,
    /// Do not print additional information nor warnings.
    #[structopt(long = "quiet", short = "q")]
    quiet: bool,
    /// Apply from-patterns recursively in the working directory tree.
    #[structopt(long = "recursive", short = "R")]
    recursive: bool,
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
    /// Moves matched files.
    Move {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
}

impl AsRef<UnparsedTransform> for Command {
    fn as_ref(&self) -> &UnparsedTransform {
        match *self {
            Command::Append { ref transform, .. } => transform,
            Command::Copy { ref transform, .. } => transform,
            Command::Move { ref transform, .. } => transform,
        }
    }
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

struct Harness {
    actuator: Actuator,
    directory: PathBuf,
    depth: usize,
    force: bool,
    quiet: bool,
}

impl Harness {
    fn execute<A>(&self, transform: &Transform) -> Result<(), Error>
    where
        A: Label + Operation,
    {
        let terminal = Term::stderr();
        let manifest: Manifest<A::Routing> = transform.read(&self.directory, self.depth)?;
        if !self.quiet {
            manifest.print(&terminal)?;
            ui::print_disclaimer(&terminal)?;
        }
        let actuate = self.force
            || ui::confirmation(
                &terminal,
                format!(
                    "Ready to {} into {} files. Continue?",
                    A::LABEL,
                    manifest.count()
                ),
            )?;
        if actuate {
            for route in manifest.routes().print_progress(terminal) {
                self.actuator.write::<A, _>(route)?;
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    let harness = Harness {
        actuator: Actuator {
            parents: options.parents,
            overwrite: options.overwrite,
        },
        directory: options.directory,
        depth: if options.recursive { usize::MAX } else { 1 },
        force: options.force,
        quiet: options.quiet,
    };
    // TODO: Parse `Transform`s with `structopt`.
    match options.command {
        Command::Append { .. } => todo!(),
        Command::Copy { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: if options.glob {
                    todo!()
                }
                else {
                    Regex::new(&transform.from)?.into()
                },
                to,
            };
            harness.execute::<Copy>(&transform)?;
        }
        Command::Move { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: if options.glob {
                    todo!()
                }
                else {
                    Regex::new(&transform.from)?.into()
                },
                to,
            };
            harness.execute::<Move>(&transform)?;
        }
    }
    Ok(())
}
