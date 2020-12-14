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

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    #[structopt(long = "", short = "-C", default_value = ".")]
    directory: PathBuf,
    #[structopt(long = "--force", short = "-f")]
    force: bool,
    #[structopt(long = "--overwrite", short = "-w")]
    overwrite: bool,
    #[structopt(long = "--parents", short = "-p")]
    parents: bool,
    #[structopt(long = "--quiet", short = "-q")]
    quiet: bool,
    #[structopt(long = "--recursive", short = "-R")]
    recursive: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    Copy {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
    Move {
        #[structopt(flatten)]
        transform: UnparsedTransform,
    },
}

impl AsRef<UnparsedTransform> for Command {
    fn as_ref(&self) -> &UnparsedTransform {
        match *self {
            Command::Copy { ref transform, .. } => transform,
            Command::Move { ref transform, .. } => transform,
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct UnparsedTransform {
    from: Regex,
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
        let mut terminal = Term::stderr();
        let manifest: Manifest<A::Routing> = transform.read(&self.directory, self.depth)?;
        if !self.quiet {
            manifest.print(&mut terminal)?;
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
            for (sources, destination) in manifest.paths().print_progress(terminal) {
                self.actuator.write::<A, _, _>(sources, destination)?;
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
        Command::Copy { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: transform.from.into(),
                to,
            };
            harness.execute::<Copy>(&transform)?;
        }
        Command::Move { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: transform.from.into(),
                to,
            };
            harness.execute::<Move>(&transform)?;
        }
    }
    Ok(())
}
