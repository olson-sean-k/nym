mod option;
mod terminal;

use anyhow::Error;
use std::path::PathBuf;
use structopt::StructOpt;

use nym::actuator::{Copy, HardLink, Move, Operation, SoftLink};
use nym::environment::{Environment, Policy};
use nym::glob::Glob;
use nym::manifest::Manifest;
use nym::pattern::{FromPattern, ToPattern};

use crate::option::{ChildCommand, Toggle};
use crate::terminal::{IteratorExt as _, Print, Terminal};

const WARNING_TRANSFORM: &str = "paths may be ambiguous and undetected collisions may cause \
                                 overwriting, truncation, and data loss; review patterns and paths \
                                 carefully.";

trait Label {
    const LABEL: &'static str;
}

impl Label for Copy {
    const LABEL: &'static str = "copy";
}

impl Label for HardLink {
    const LABEL: &'static str = "hard link";
}

impl Label for Move {
    const LABEL: &'static str = "move";
}

impl Label for SoftLink {
    const LABEL: &'static str = "soft link";
}

/// Append, copy, link, and move files using patterns.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Program {
    #[structopt(subcommand)]
    command: Command,
}

impl Program {
    pub fn run(&mut self) -> Result<(), Error> {
        terminal::toggle_color_output(self.command.common_option_group().color);
        match self.command {
            Command::Append { .. } => todo!("append"),
            Command::Copy {
                ref mut options,
                ref transform,
                ..
            } => actuate::<Copy>(options, transform),
            Command::Find {
                ref mut options,
                ref from,
                ..
            } => {
                let from = FromPattern::from(Glob::partitioned(from)?);
                let mut output = Terminal::with_output_process(&mut options.pager, options.paging);
                for entry in from.read(&options.directory, options.depth + 1).flatten() {
                    entry.path().print(&mut output)?;
                }
                Ok(())
            }
            Command::Link { ref mut link, .. } => match link {
                Link::Hard {
                    ref mut options,
                    ref transform,
                    ..
                } => actuate::<HardLink>(options, transform),
                Link::Soft {
                    ref mut options,
                    ref transform,
                    ..
                } => actuate::<SoftLink>(options, transform),
            },
            Command::Move {
                ref mut options,
                ref transform,
                ..
            } => actuate::<Move>(options, transform),
        }
    }
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct CommonOptionGroup {
    /// Working directory tree.
    #[structopt(long = "tree", short = "C", default_value = ".")]
    directory: PathBuf,
    /// Maximum depth traversed into the working directory tree.
    ///
    /// A depth of zero only includes files within the working directory (there
    /// is no traversal into directories).
    #[structopt(long = "depth", default_value = "255")]
    depth: usize,
    /// Determines if and when non-error output is routed to a configured pager.
    ///
    /// One of "always", "never", or "automatic" (or its abbreviation "auto").
    /// When "automatic", output is only routed to the configured pager if
    /// standard output is attached to an attended terminal (not piped,
    /// redirected, etc.).
    #[structopt(long = "paging", value_name = "when", default_value = "automatic")]
    paging: Toggle,
    /// Pager command line.
    #[structopt(
        long = "pager",
        value_name = "command",
        default_value = "less -R --no-init --quit-if-one-screen --quit-on-intr"
    )]
    pager: ChildCommand,
    /// Determines if and when color and style is enabled in output.
    ///
    /// One of "always", "never", or "automatic" (or its abbreviation "auto").
    /// When "automatic", output is colored and styled based on the CLI colors
    /// specification: https://bixense.com/clicolors/
    #[structopt(long = "color", value_name = "when", default_value = "automatic")]
    color: Toggle,
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct TransformOptionGroup {
    #[structopt(flatten)]
    common: CommonOptionGroup,
    /// Determines if and when interactive prompts are used.
    ///
    /// One of "always", "never", or "automatic" (or its abbreviation "auto").
    /// When "automatic", prompts are used if standard error is attached to an
    /// attended terminal (not piped, redirected, etc.).
    ///
    /// Note that if standard error is piped or redirected and this option is
    /// "always", then prompts will default to taking no action and commands
    /// will never be executed.
    #[structopt(long = "interactive", value_name = "when", default_value = "always")]
    interactive: Toggle,
    /// Do not print manifests nor warnings.
    #[structopt(long = "quiet", short = "q")]
    quiet: bool,
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
        options: TransformOptionGroup,
    },
    /// Copies matched files.
    Copy {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptionGroup,
    },
    /// Finds matched files.
    Find {
        /// The from-pattern used to match files.
        from: String,
        #[structopt(flatten)]
        options: CommonOptionGroup,
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
        options: TransformOptionGroup,
    },
}

impl Command {
    fn common_option_group(&self) -> &CommonOptionGroup {
        match self {
            Command::Append { ref options, .. }
            | Command::Copy { ref options, .. }
            | Command::Move { ref options, .. } => &options.common,
            Command::Link { ref link, .. } => match link {
                Link::Hard { ref options, .. } | Link::Soft { ref options, .. } => &options.common,
            },
            Command::Find { ref options, .. } => options,
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
        options: TransformOptionGroup,
    },
    /// Symbolically links matched files.
    Soft {
        #[structopt(flatten)]
        transform: UnparsedTransform,
        #[structopt(flatten)]
        options: TransformOptionGroup,
    },
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
        let from = Glob::partitioned(&self.from)?.into();
        let to = ToPattern::new(&self.to)?;
        Ok((from, to))
    }
}

fn actuate<A>(
    options: &mut TransformOptionGroup,
    transform: &UnparsedTransform,
) -> Result<(), Error>
where
    A: Label + Operation,
{
    let environment = Environment::new(Policy {
        parents: options.parents,
        overwrite: options.overwrite,
    });
    let (from, to) = transform.parse()?;

    let transform = environment.transform(from, to);
    let actuator = environment.actuator();
    let manifest: Manifest<A::Routing> =
        transform.read(&options.common.directory, options.common.depth + 1)?;

    if !options.quiet {
        Terminal::with_output_process_scoped(
            &mut options.common.pager,
            options.common.paging,
            |mut output| manifest.print(&mut output),
        )?;
        terminal::warning(WARNING_TRANSFORM)?;
    }
    if !terminal::is_interactive(options.interactive)
        || terminal::confirm(format!(
            "Ready to {} into {} files. Continue?",
            A::LABEL,
            manifest.routes().len(),
        ))?
    {
        for route in manifest.routes().printed() {
            actuator.write::<A, _>(route)?;
        }
    }
    Ok(())
}

fn main() -> Result<(), Error> {
    Program::from_args().run()
}
