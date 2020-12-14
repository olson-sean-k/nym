mod ui;

use anyhow::Error;
use console::Term;
use fool::or;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Actuator, Copy, Move};
use nym::manifest::Manifest;
use nym::pattern::ToPattern;
use nym::transform::Transform;

use crate::ui::IteratorExt as _;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    #[structopt(long = "--working-dir", short = "-C", default_value = ".")]
    directory: PathBuf,
    #[structopt(long = "--recursive", short = "-R")]
    recursive: bool,
    #[structopt(long = "--force", short = "-f")]
    force: bool,
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

struct Executor {
    directory: PathBuf,
    depth: usize,
    force: bool,
}

impl Executor {
    fn execute<A>(&self, transform: &Transform) -> Result<(), Error>
    where
        A: Actuator,
    {
        let mut terminal = Term::stderr();
        let manifest: A::Manifest = transform.read(&self.directory, self.depth)?;
        let paths = manifest.into_grouped_paths();
        ui::print_grouped_paths(&mut terminal, &paths)?;
        if or!(
            self.force,
            ui::confirmation(
                &terminal,
                format!("Ready to {} into {} files. Continue?", A::NAME, paths.len()),
            )?,
        ) {
            for (sources, destination) in paths.into_iter().print_actuator_progress(terminal) {
                A::write(sources, destination)?;
            }
        }
        Ok(())
    }
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    let executor = Executor {
        directory: options.directory,
        depth: if options.recursive { usize::MAX } else { 1 },
        force: options.force,
    };
    match options.command {
        Command::Copy { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: transform.from.into(),
                to,
            };
            executor.execute::<Copy>(&transform)?;
        }
        Command::Move { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: transform.from.into(),
                to,
            };
            executor.execute::<Move>(&transform)?;
        }
    }
    Ok(())
}
