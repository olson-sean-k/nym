use std::ffi::OsStr;
use std::io::{self, Write};
use std::process::{Child, Command, Stdio};
use std::str::FromStr;
use thiserror::Error;

pub trait ResultExt<T, E>: Sized {
    fn broken_pipe_ok(self, value: T) -> Self {
        self.broken_pipe_ok_with(move || value)
    }

    fn broken_pipe_ok_with<F>(self, f: F) -> Self
    where
        F: FnOnce() -> T;
}

impl<T> ResultExt<T, io::Error> for Result<T, io::Error> {
    fn broken_pipe_ok_with<F>(self, f: F) -> Self
    where
        F: FnOnce() -> T,
    {
        match self {
            Err(error) => {
                if matches!(error.kind(), io::ErrorKind::BrokenPipe) {
                    Ok(f())
                }
                else {
                    Err(error)
                }
            }
            _ => self,
        }
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OptionError {
    #[error("failed to parse option")]
    Parse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Toggle {
    Always,
    Automatic,
    Never,
}

impl FromStr for Toggle {
    type Err = OptionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "always" => Ok(Toggle::Always),
            "auto" | "automatic" => Ok(Toggle::Automatic),
            "never" => Ok(Toggle::Never),
            _ => Err(OptionError::Parse),
        }
    }
}

impl Default for Toggle {
    fn default() -> Self {
        Toggle::Automatic
    }
}

#[derive(Debug)]
pub struct Wait {
    child: Child,
}

impl Drop for Wait {
    fn drop(&mut self) {
        let _ = self.child.wait();
    }
}

impl Write for Wait {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if let Some(ref mut stdin) = self.child.stdin {
            // NOTE: If the pipe is closed, the `buffer` is never actually
            //       written. This returns `Ok` with the buffer length rather
            //       than `Ok(0)`, because `Ok(0)` may cause downstream errors.
            stdin.write(buffer).broken_pipe_ok(buffer.len())
        }
        else {
            Ok(0) // This may cause a downstream error.
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut stdin) = self.child.stdin {
            stdin.flush().broken_pipe_ok(())
        }
        else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub struct ChildCommand {
    command: Command,
}

impl ChildCommand {
    pub fn from_command<I>(binary: impl AsRef<OsStr>, arguments: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<OsStr>,
    {
        let mut command = Command::new(binary);
        command.args(arguments).stdin(Stdio::piped());
        ChildCommand { command }
    }

    pub fn wait(&mut self) -> io::Result<Wait> {
        let child = self.command.spawn()?;
        Ok(Wait { child })
    }
}

impl FromStr for ChildCommand {
    type Err = OptionError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut components = text.split_whitespace();
        let binary = components.next().ok_or(OptionError::Parse)?;
        Ok(ChildCommand::from_command(binary, components))
    }
}
