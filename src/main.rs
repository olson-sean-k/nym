mod ui;

use anyhow::Error;
use console::Term;
use regex::bytes::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Actuator, Copy, Move, Operation};
use nym::glob::Glob;
use nym::manifest::Manifest;
use nym::pattern::ToPattern;
use nym::policy::Policy;
use nym::transform::Transform;

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
    /// The working directory.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
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
    fn parse(&self, is_regex: bool) -> Result<Transform<'_, '_>, Error> {
        Ok(Transform {
            from: if is_regex {
                Regex::new(&self.from)?.into()
            }
            else {
                Glob::parse(&self.from)?.into()
            },
            to: ToPattern::parse(&self.to)?,
        })
    }
}

struct Harness {
    actuator: Actuator,
    policy: Policy,
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
        let manifest: Manifest<A::Router> =
            transform.read(&self.policy, &self.directory, self.depth)?;
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
                self.actuator.write::<A, _>(&self.policy, route)?;
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    let harness = Harness {
        actuator: Actuator::default(),
        policy: Policy {
            parents: options.parents,
            overwrite: options.overwrite,
        },
        directory: options.directory,
        depth: if options.recursive { usize::MAX } else { 1 },
        force: options.force,
        quiet: options.quiet,
    };
    match options.command {
        Command::Append { .. } => todo!(),
        Command::Copy { transform, .. } => {
            let transform = transform.parse(options.regex)?;
            harness.execute::<Copy>(&transform)?;
        }
        Command::Link { .. } => todo!(),
        Command::Move { transform, .. } => {
            let transform = transform.parse(options.regex)?;
            harness.execute::<Move>(&transform)?;
        }
    }
    Ok(())
}
