use console::{self, Style, Term};
use dialoguer::theme::ColorfulTheme;
use dialoguer::Confirm;
use indicatif::{ProgressBar, ProgressBarIter, ProgressDrawTarget, ProgressIterator};
use itertools::{Itertools as _, Position};
use lazy_static::lazy_static;
use lscolors::{self, LsColors};
use std::cmp;
use std::convert::{TryFrom, TryInto};
use std::io::{self, Read, Write};
use std::path::Path;

use nym::manifest::{Manifest, Routing};

use crate::option::{ChildCommand, Toggle, Wait};

const MIN_TERMINAL_WIDTH: usize = 16;

lazy_static! {
    static ref STYLE_INDEX: Style = Style::new().bright().white();
    static ref STYLE_LINE: Style = Style::new();
    static ref STYLE_SOURCE_PATH: Style = Style::new().green();
    static ref STYLE_DESTINATION_PATH: Style = Style::new().red();
    static ref STYLE_WARNING: Style = Style::new().bold();
    static ref STYLE_WARNING_HEADER: Style = Style::new().blink().bold().yellow();
}

pub trait FromStyle<T>: Sized {
    fn from_style(style: T) -> Self;
}

pub trait IntoStyle<T>: Sized {
    fn into_style(self) -> T;
}

impl<T, U> IntoStyle<T> for U
where
    T: FromStyle<U>,
{
    fn into_style(self) -> T {
        T::from_style(self)
    }
}

impl FromStyle<lscolors::Style> for Style {
    fn from_style(style: lscolors::Style) -> Self {
        use lscolors::Color;

        fn set_if(style: Style, condition: bool, f: impl FnOnce(Style) -> Style) -> Style {
            if condition {
                f(style)
            }
            else {
                style
            }
        }

        let lscolors::Style {
            background,
            foreground,
            font_style: attributes,
            ..
        } = style;
        let mut style = Style::default();
        if let Some(background) = background {
            style = match background {
                Color::Black => style.on_black(),
                Color::Red => style.on_red(),
                Color::Green => style.on_green(),
                Color::Yellow => style.on_yellow(),
                Color::Blue => style.on_blue(),
                Color::Magenta => style.on_magenta(),
                Color::Cyan => style.on_cyan(),
                Color::White => style.on_white(),
                Color::Fixed(color) => style.on_color256(color),
                Color::RGB(_, _, _) => style,
            }
        }
        if let Some(foreground) = foreground {
            style = match foreground {
                Color::Black => style.black(),
                Color::Red => style.red(),
                Color::Green => style.green(),
                Color::Yellow => style.yellow(),
                Color::Blue => style.blue(),
                Color::Magenta => style.magenta(),
                Color::Cyan => style.cyan(),
                Color::White => style.white(),
                Color::Fixed(color) => style.color256(color),
                Color::RGB(_, _, _) => style,
            }
        }
        style = set_if(
            style,
            attributes.slow_blink || attributes.rapid_blink,
            |style| style.blink(),
        );
        style = set_if(style, attributes.bold, |style| style.bold());
        style = set_if(style, attributes.dimmed, |style| style.dim());
        style = set_if(style, attributes.hidden, |style| style.hidden());
        style = set_if(style, attributes.italic, |style| style.italic());
        style = set_if(style, attributes.reverse, |style| style.reverse());
        style = set_if(style, attributes.underline, |style| style.underlined());
        style
    }
}

pub trait Page {
    fn layout(&self) -> Option<Layout>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    width: usize,
    height: usize,
}

impl Layout {
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }
}

#[derive(Debug)]
enum Output {
    Terminal,
    Process(Wait),
}

#[derive(Debug)]
pub struct Terminal {
    inner: Term,
    output: Output,
}

impl Terminal {
    pub fn with_output_process(command: &mut ChildCommand, toggle: Toggle) -> Self {
        match toggle {
            Toggle::Always => command.try_into().unwrap_or_else(|_| Term::stdout().into()),
            Toggle::Automatic => {
                let terminal = Term::stdout();
                if terminal.features().is_attended() {
                    command.try_into().unwrap_or_else(|_| Term::stdout().into())
                }
                else {
                    terminal.into()
                }
            }
            Toggle::Never => Term::stdout().into(),
        }
    }

    pub fn with_output_process_scoped<T, F>(
        command: &mut ChildCommand,
        toggle: Toggle,
        mut f: F,
    ) -> T
    where
        F: FnMut(Terminal) -> T,
    {
        f(Self::with_output_process(command, toggle))
    }
}

impl Default for Terminal {
    fn default() -> Self {
        Term::stdout().into()
    }
}

impl From<Term> for Terminal {
    fn from(terminal: Term) -> Self {
        Terminal {
            inner: terminal,
            output: Output::Terminal,
        }
    }
}

impl Page for Terminal {
    fn layout(&self) -> Option<Layout> {
        self.inner.features().is_attended().then(|| {
            let (height, width) = self.inner.size();
            Layout {
                width: usize::try_from(width).expect("width overflow"),
                height: usize::try_from(height).expect("height overflow"),
            }
        })
    }
}

impl Read for Terminal {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buffer)
    }
}

impl<'p> TryFrom<&'p mut ChildCommand> for Terminal {
    type Error = io::Error;

    fn try_from(command: &'p mut ChildCommand) -> io::Result<Self> {
        command.wait().map(|wait| Terminal {
            inner: Term::stdout(),
            output: Output::Process(wait),
        })
    }
}

impl Write for Terminal {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        match self.output {
            Output::Terminal => self.inner.write(buffer),
            Output::Process(ref mut child) => child.write(buffer),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.output {
            Output::Terminal => self.inner.flush(),
            Output::Process(ref mut child) => child.flush(),
        }
    }
}

pub trait IteratorExt: Iterator + Sized {
    fn printed(self) -> ProgressBarIter<Self>
    where
        Self: ExactSizeIterator,
    {
        let n = u64::try_from(self.len()).expect("length overflow");
        self.progress_with(ProgressBar::with_draw_target(
            n,
            ProgressDrawTarget::stderr(),
        ))
    }
}

impl<I> IteratorExt for I where I: Iterator + Sized {}

pub trait Stylize {
    fn stylize(&self) -> Vec<u8> {
        let mut output = Vec::new();
        self.stylize_into(&mut output).expect("");
        output
    }

    fn stylize_into(&self, output: &mut impl Write) -> io::Result<()>;
}

impl<'p> Stylize for &'p Path {
    // TODO: This reads file metadata regardless of whether or not color is
    //       enabled. If color is disabled, do not read metadata.
    // TODO: `LS_COLORS` is only used by the `find` sub-command, but it could
    //       be useful elsewhere. However, text wrapping and other formatting
    //       must be aware of ANSI escape codes and `textwrap` is not.
    //       Implement a way to format stylized outputs.
    fn stylize_into(&self, output: &mut impl Write) -> io::Result<()> {
        let colors = LsColors::from_env().unwrap_or_default();
        for (text, style) in colors.style_for_path_components(*self) {
            let style = style.cloned().map(Style::from_style).unwrap_or_default();
            write!(output, "{}", style.apply_to(text.to_string_lossy()))?;
        }
        Ok(())
    }
}

pub trait Print {
    fn print(&self, output: &mut (impl Page + Write)) -> io::Result<()>;
}

impl<'p> Print for &'p Path {
    fn print(&self, output: &mut (impl Page + Write)) -> io::Result<()> {
        self.stylize_into(output)?;
        writeln!(output)
    }
}

impl<M> Print for Manifest<M>
where
    M: Routing,
{
    fn print(&self, output: &mut (impl Page + Write)) -> io::Result<()> {
        let routes = self.routes();
        let margin = ((routes.len() as f64).log10() as usize) + 1;
        let width = width(output, margin + 6);
        for (n, route) in routes.enumerate() {
            for source in route.sources().with_position() {
                match source {
                    Position::First(source) | Position::Only(source) => {
                        let source = source.to_string_lossy();
                        for line in textwrap::wrap(source.as_ref(), width)
                            .into_iter()
                            .with_position()
                        {
                            match line {
                                Position::First(line) | Position::Only(line) => writeln!(
                                    output,
                                    "{:0>width$} {} {}",
                                    STYLE_INDEX.apply_to(n + 1),
                                    STYLE_LINE.apply_to("─┬──"),
                                    STYLE_SOURCE_PATH.apply_to(line),
                                    width = margin,
                                ),
                                Position::Middle(line) | Position::Last(line) => writeln!(
                                    output,
                                    "{: >width$}   {}",
                                    STYLE_LINE.apply_to("│"),
                                    STYLE_SOURCE_PATH.apply_to(line),
                                    width = margin + 3,
                                ),
                            }?;
                        }
                    }
                    Position::Middle(source) | Position::Last(source) => {
                        let source = source.to_string_lossy();
                        for line in textwrap::wrap(source.as_ref(), width)
                            .into_iter()
                            .with_position()
                        {
                            match line {
                                Position::First(line) | Position::Only(line) => writeln!(
                                    output,
                                    "{: >width$} {}",
                                    STYLE_LINE.apply_to("├──"),
                                    STYLE_SOURCE_PATH.apply_to(line),
                                    width = margin + 3,
                                ),
                                Position::Middle(line) | Position::Last(line) => writeln!(
                                    output,
                                    "{: >width$}   {}",
                                    STYLE_LINE.apply_to("│"),
                                    STYLE_SOURCE_PATH.apply_to(line),
                                    width = margin + 3,
                                ),
                            }?;
                        }
                    }
                }
            }
            let destination = route.destination().to_string_lossy();
            for line in textwrap::wrap(destination.as_ref(), width)
                .into_iter()
                .with_position()
            {
                match line {
                    Position::First(line) | Position::Only(line) => writeln!(
                        output,
                        "{: >width$} {}",
                        STYLE_LINE.apply_to("╰─⯈"),
                        STYLE_DESTINATION_PATH.apply_to(line),
                        width = margin + 5,
                    ),
                    Position::Middle(line) | Position::Last(line) => writeln!(
                        output,
                        "{: >width$}{}",
                        "",
                        STYLE_DESTINATION_PATH.apply_to(line),
                        width = margin + 6,
                    ),
                }?;
            }
        }
        Ok(())
    }
}

pub fn warning(warning: impl AsRef<str>) -> io::Result<()> {
    const HEADER: &str = "Warning";

    let mut output = Terminal::from(Term::stderr());
    let margin = HEADER.len() + 2;
    for line in textwrap::wrap(warning.as_ref(), width(&output, margin))
        .into_iter()
        .with_position()
    {
        match line {
            Position::First(line) | Position::Only(line) => writeln!(
                output,
                "{}{} {}",
                STYLE_WARNING_HEADER.apply_to(HEADER),
                STYLE_WARNING.apply_to(":"),
                STYLE_WARNING.apply_to(line),
            ),
            Position::Middle(line) | Position::Last(line) => writeln!(
                output,
                "{: <width$}{}",
                "",
                STYLE_WARNING.apply_to(line),
                width = margin,
            ),
        }?;
    }
    Ok(())
}

// NOTE: This fails if used with an unattended terminal. This prevents shell
//       redirects from bypassing confirmation prompts, but means that
//       redirecting `stderr` requires the `--force` flag.
pub fn confirm(prompt: impl AsRef<str>) -> io::Result<bool> {
    Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt.as_ref())
        .default(false)
        .show_default(true)
        .wait_for_newline(true)
        .interact()
}

pub fn is_interactive(toggle: Toggle) -> bool {
    match toggle {
        Toggle::Always => true,
        Toggle::Automatic => Term::stderr().features().is_attended(),
        Toggle::Never => false,
    }
}

pub fn toggle_color_output(toggle: Toggle) {
    let (output, error) = match toggle {
        Toggle::Always => (true, true),
        Toggle::Automatic => {
            // TODO: `console` does not provide a way to re-enable its
            //       heuristics for detecting color support. At the time of this
            //       writing, terminal features always report that color output
            //       is supported, so this case does nothing. Note that any
            //       subsequent calls to this function with `Toggle::Automatic`
            //       will not behave as expected if previously called with
            //       `Toggle::Always` or `Toggle::Never`.
            return;
        }
        Toggle::Never => (false, false),
    };
    console::set_colors_enabled(output);
    console::set_colors_enabled_stderr(error);
}

fn width(output: &impl Page, margin: usize) -> usize {
    if let Some(layout) = output.layout() {
        let (width, _) = layout.dimensions();
        cmp::max(width - cmp::min(width, margin), MIN_TERMINAL_WIDTH)
    }
    else {
        usize::MAX - 1
    }
}
