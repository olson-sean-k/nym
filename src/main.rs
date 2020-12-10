use anyhow::Error;
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Actuator, Move};
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
        let manifest: A::Manifest = transform.read(&self.directory, self.depth)?;
        print!("{}", manifest);
        if self.force
            || Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt("Continue?")
                .default(false)
                .show_default(true)
                .wait_for_newline(true)
                .interact()?
        {
            //println!("{:?}", transform);
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
        Command::Move { transform, .. } => {
            let to = ToPattern::parse(&transform.to)?;
            let transform = Transform {
                from: transform.from.into(),
                to,
            };
            executor.execute::<Move>(&transform)?;
        }
        _ => {}
    }
    Ok(())
}
