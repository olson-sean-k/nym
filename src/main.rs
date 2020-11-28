use anyhow::Error;
use bimap::BiMap;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::pattern::Pattern;
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
    #[structopt(long = "--help", short = "-h")]
    help: bool,
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

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    match options.command {
        Command::Move { transform, .. } => {
            let to = Pattern::parse(&transform.to)?;
            println!("{:?} -> {:?}", transform.from, to);
            let transform = Transform {
                from: transform.from,
                to,
            };
            let manifest: BiMap<_, _> = transform.scan(options.directory, usize::MAX)?;
            println!("{:?}", manifest);
        }
        _ => {}
    }
    Ok(())
}
