use anyhow::Error;
use regex::Regex;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::Pattern;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Options {
    #[structopt(subcommand)]
    command: Command,
    #[structopt(long = "--help", short = "-h")]
    help: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    Copy {
        #[structopt(flatten)]
        immediate: Immediate,
    },
    #[cfg(feature = "edit")]
    Edit,
    Move {
        #[structopt(flatten)]
        immediate: Immediate,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Immediate {
    #[structopt(long = "--working-dir", short = "-C", default_value = ".")]
    directory: PathBuf,
    #[structopt(flatten)]
    transform: Transform,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Transform {
    #[structopt(long = "--recursive", short = "-R")]
    recursive: bool,
    from: Regex,
    to: String,
}

fn main() -> Result<(), Error> {
    let options = Options::from_args();
    match options.command {
        #[cfg(feature = "edit")]
        Command::Edit => {
            use std::io;

            use nym::edit::Edit;

            let mut edit = Edit::attach(io::stdout())?;
            edit.execute()?;
        }
        Command::Move { immediate: Immediate { transform, .. }, .. } => {
            let Transform { from, to, .. } = transform;
            let to = Pattern::parse(&to)?;
            println!("{:?} -> {:?}", from, to);
        }
        _ => {}
    }
    Ok(())
}
