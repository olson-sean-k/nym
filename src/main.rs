mod ui;

use anyhow::Error;
use console::Term;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Copy, HardLink, Move, Operation, SoftLink};
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
struct Program {
    #[structopt(subcommand)]
    command: Command,
    /// Working directory tree.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
    /// Maximum depth traversed into the working directory tree.
    ///
    /// A depth of zero only includes files within the working directory (there
    /// is no traversal into directories).
    #[structopt(long = "depth", default_value = "255")]
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

impl Program {
    pub fn run(&self) -> Result<(), Error> {
        let environment = Environment::new(Policy {
            parents: self.parents,
            overwrite: self.overwrite,
        });
        match self.command {
            Command::Append { .. } => todo!("append"),
            Command::Copy { ref transform, .. } => {
                let (from, to) = transform.parse()?;
                self.actuate::<Copy>(environment, from, to)?;
            }
            Command::Link { ref link, .. } => match link {
                Link::Hard { ref transform, .. } => {
                    let (from, to) = transform.parse()?;
                    self.actuate::<HardLink>(environment, from, to)?;
                }
                Link::Soft { ref transform, .. } => {
                    let (from, to) = transform.parse()?;
                    self.actuate::<SoftLink>(environment, from, to)?;
                }
            },
            Command::Move { ref transform, .. } => {
                let (from, to) = transform.parse()?;
                self.actuate::<Move>(environment, from, to)?;
            }
        }
        Ok(())
    }

    fn actuate<A>(
        &self,
        environment: Environment,
        from: FromPattern<'_>,
        to: ToPattern<'_>,
    ) -> Result<(), Error>
    where
        A: Label + Operation,
    {
        let terminal = Term::stderr();
        let transform = environment.transform(from, to);
        let actuator = environment.actuator();
        let manifest: Manifest<A::Routing> = transform.read(&self.directory, self.depth + 1)?;
        if !self.quiet {
            manifest.print(&terminal)?;
            ui::print_warning(&terminal, DISCLAIMER)?;
        }
        if self.force
            || ui::confirm(
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
    /// Links matched files.
    Link {
        #[structopt(subcommand)]
        link: Link,
    },
    /// Moves matched files.
    Move {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Link {
    /// Hard links matched files.
    Hard {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
    /// Symbolically links matched files.
    Soft {
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
    fn parse(&self) -> Result<(FromPattern<'_>, ToPattern<'_>), Error> {
        let from = Glob::parse(&self.from)?.into();
        let to = ToPattern::parse(&self.to)?;
        Ok((from, to))
    }
}

fn main() -> Result<(), Error> {
    Program::from_args().run()
}
