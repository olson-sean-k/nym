mod edit;

use regex::Regex;
use std::io;
use structopt::StructOpt;

#[cfg(feature = "edit")]
use crate::edit::Editor;

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
        transform: Transform,
    },
    #[cfg(feature = "edit")]
    Edit,
    Move {
        #[structopt(flatten)]
        transform: Transform,
    },
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Transform {
    from: Regex,
    // TODO: Provide a type for referencing matches that implements `FromStr`.
    to: String,
}

fn main() {
    let options = Options::from_args();
    #[cfg(feature = "edit")]
    if let Command::Edit = options.command {
        let mut editor = Editor::attach(io::stdout()).unwrap();
        let _ = editor.run();
    }
}
