use anyhow::Error;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::transform::Transform;
use nym::pattern::Pattern;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    #[structopt(long = "--working-dir", short = "-C", default_value = ".")]
    directory: PathBuf,
    #[structopt(long = "--recursive", short = "-R")]
    recursive: bool,
    #[structopt(long = "--help", short = "-h")]
    help: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    Copy {
        #[structopt(flatten)]
        targets: CommandTargets,
    },
    Move {
        #[structopt(flatten)]
        targets: CommandTargets,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct CommandTargets {
    from: Regex,
    to: String,
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    match options.command {
        Command::Move { targets, .. } => {
            let to = Pattern::parse(&targets.to)?;
            println!("{:?} -> {:?}", targets.from, to);
            let transform = Transform {
                from: targets.from,
                to,
            };
            let renames = transform.scan(options.directory)?;
            println!("{:?}", renames);
        }
        _ => {}
    }
    Ok(())
}
