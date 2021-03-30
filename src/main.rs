mod ui;

use anyhow::Error;
use console::Term;
use std::io::{self, Write};
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Copy, HardLink, Move, Operation, SoftLink};
use nym::environment::{Environment, Policy};
use nym::glob::Glob;
use nym::manifest::Manifest;
use nym::pattern::{FromPattern, ToPattern};

use crate::ui::{IteratorExt as _, Label, Print};

const DISCLAIMER: &str = "paths may be ambiguous and undetected collisions may cause overwriting, \
                          truncation, and data loss; review patterns and paths carefully.";

/// Append, copy, link, and move files using patterns.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Program {
    #[structopt(subcommand)]
    command: Command,
}

impl Program {
    pub fn run(&self) -> Result<(), Error> {
        match self.command {
            Command::Append { .. } => todo!("append"),
            Command::Copy {
                ref options,
                ref transform,
                ..
            } => with_transform(options, transform, |environment, from, to| {
                actuate::<Copy>(self.options(), environment, from, to)
            }),
            // TODO: Use `console` and the `ui` module to handle output.
            Command::Find {
                ref options,
                ref from,
                ..
            } => {
                let from = FromPattern::from(Glob::parse(from)?);
                let out = io::stderr();
                let mut out = out.lock();
                for entry in from.read(&options.directory, options.depth + 1) {
                    if let Ok(entry) = entry {
                        let _ = writeln!(out, "{}", entry.path().to_string_lossy().as_ref());
                    }
                }
                Ok(())
            }
            Command::Link { ref link, .. } => match link {
                Link::Hard {
                    ref options,
                    ref transform,
                    ..
                } => with_transform(options, transform, |environment, from, to| {
                    actuate::<HardLink>(self.options(), environment, from, to)
                }),
                Link::Soft {
                    ref options,
                    ref transform,
                    ..
                } => with_transform(options, transform, |environment, from, to| {
                    actuate::<SoftLink>(self.options(), environment, from, to)
                }),
            },
            Command::Move {
                ref options,
                ref transform,
                ..
            } => with_transform(options, transform, |environment, from, to| {
                actuate::<Move>(self.options(), environment, from, to)
            }),
        }
    }

    fn options(&self) -> &CommonOptions {
        self.command.options()
    }
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct CommonOptions {
    /// Working directory tree.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
    /// Maximum depth traversed into the working directory tree.
    ///
    /// A depth of zero only includes files within the working directory (there
    /// is no traversal into directories).
    #[structopt(long = "depth", default_value = "255")]
    depth: usize,
    /// Perform operations without interactive prompts and ignoring warnings.
    #[structopt(long = "force", short = "f")]
    force: bool,
    /// Do not print additional information nor warnings.
    #[structopt(long = "quiet", short = "q")]
    quiet: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct TransformOptions {
    #[structopt(flatten)]
    options: CommonOptions,
    /// Overwrite existing files resolved by to-patterns.
    #[structopt(long = "overwrite", short = "w")]
    overwrite: bool,
    /// Create parent directories for paths resolved by to-patterns.
    #[structopt(long = "parents", short = "p")]
    parents: bool,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    /// Appends matched files.
    Append {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptions,
    },
    /// Copies matched files.
    Copy {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptions,
    },
    /// Finds matched files.
    Find {
        /// The from-pattern used to match files.
        from: String,
        #[structopt(flatten)]
        options: CommonOptions,
    },
    /// Links matched files.
    Link {
        #[structopt(subcommand)]
        link: Link,
    },
    /// Moves matched files.
    Move {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptions,
    },
}

impl Command {
    fn options(&self) -> &CommonOptions {
        match self {
            Command::Append {
                options: TransformOptions { ref options, .. },
                ..
            } => options,
            Command::Copy {
                options: TransformOptions { ref options, .. },
                ..
            } => options,
            Command::Find { ref options, .. } => options,
            Command::Link { ref link, .. } => link.options(),
            Command::Move {
                options: TransformOptions { ref options, .. },
                ..
            } => options,
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Link {
    /// Hard links matched files.
    Hard {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptions,
    },
    /// Symbolically links matched files.
    Soft {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptions,
    },
}

impl Link {
    fn options(&self) -> &CommonOptions {
        match self {
            Link::Hard {
                options: TransformOptions { ref options, .. },
                ..
            } => options,
            Link::Soft {
                options: TransformOptions { ref options, .. },
                ..
            } => options,
        }
    }
}

/// Transformation.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct UnparsedTransform {
    /// The from-pattern used to match source files.
    from: String,
    /// The to-pattern used to resolve destination files.
    to: String,
}

impl UnparsedTransform {
    fn parse(&self) -> Result<(FromPattern<'_>, ToPattern<'_>), Error> {
        let from = Glob::parse(&self.from)?.into();
        let to = ToPattern::parse(&self.to)?;
        Ok((from, to))
    }
}

fn with_transform<T, F>(
    options: &TransformOptions,
    transform: &UnparsedTransform,
    mut f: F,
) -> Result<T, Error>
where
    F: FnMut(Environment, FromPattern, ToPattern) -> Result<T, Error>,
{
    let environment = Environment::new(Policy {
        parents: options.parents,
        overwrite: options.overwrite,
    });
    let (from, to) = transform.parse()?;
    f(environment, from, to)
}

fn actuate<A>(
    options: &CommonOptions,
    environment: Environment,
    from: FromPattern<'_>,
    to: ToPattern<'_>,
) -> Result<(), Error>
where
    A: Label + Operation,
{
    let terminal = Term::stderr();
    let transform = environment.transform(from, to);
    let actuator = environment.actuator();
    let manifest: Manifest<A::Routing> = transform.read(&options.directory, options.depth + 1)?;
    if !options.quiet {
        manifest.print(&terminal)?;
        ui::print_warning(&terminal, DISCLAIMER)?;
    }
    if options.force
        || ui::confirm(
            &terminal,
            format!(
                "Ready to {} into {} files. Continue?",
                A::LABEL,
                manifest.routes().len(),
            ),
        )?
    {
        for route in manifest.routes().print_progress(terminal) {
            actuator.write::<A, _>(route)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    Program::from_args().run()
}
