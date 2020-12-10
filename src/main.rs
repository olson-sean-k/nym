use anyhow::Error;
use console::Term;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Actuator, Copy, Move};
use nym::manifest::Manifest;
use nym::pattern::ToPattern;
use nym::transform::Transform;

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
        manifest.print(&mut terminal)?;
        if self.force
            || Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Continue?")
                .default(false)
                .show_default(true)
                .wait_for_newline(true)
                .interact_on(&terminal)?
        {
            for (source, destination) in manifest {
                A::write(&source, &destination)?;
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
