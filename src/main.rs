use anyhow::Error;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::transform::Transform;
use nym::Pattern;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    from: Regex,
    to: String,
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
    Copy,
    Move,
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    match options.command {
        Command::Move => {
            let to = Pattern::parse(&options.to)?;
            println!("{:?} -> {:?}", options.from, to);
            let transform = Transform {
                from: options.from,
                to,
            };
            let renames = transform.scan(options.directory)?;
            println!("{:?}", renames);
        }
        _ => {}
    }
    Ok(())
}
