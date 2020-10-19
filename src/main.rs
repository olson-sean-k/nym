use regex::Regex;
use structopt::StructOpt;

#[cfg(feature = "edit")]
use nym::edit::Edit;
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
    to: String,
}

fn main() {
    let options = Options::from_args();
    match options.command {
        #[cfg(feature = "edit")]
        Command::Edit => {
            use std::io;

            let mut edit = Edit::attach(io::stdout()).unwrap();
            let _ = edit.execute();
        }
        Command::Move { transform } => {
            let Transform { from, to } = transform;
            let to = Pattern::parse(&to).unwrap();
            println!("{:?}", from);
            println!("{:?}", to);
        }
        _ => {}
    }
}
