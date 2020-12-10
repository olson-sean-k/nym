use bimap::BiMap;
use normpath::PathExt as _;
use std::cmp;
use std::fmt::{self, Display, Formatter};
use std::io::{self, Error, ErrorKind};
use std::iter;
use std::path::{Path, PathBuf};
use textwrap;
use walkdir::WalkDir;

use crate::pattern::{FromPattern, ToPattern};

pub trait Manifest: Default + Display + IntoIterator<Item = (PathBuf, PathBuf)> {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()>;
}

#[derive(Clone, Debug, Default)]
pub struct Bijective {
    inner: BiMap<PathBuf, PathBuf>,
}

// TODO: Do not use `Display` to print manifests. Instead, use a more specific
//       function that interacts with a `console::Term` rather than a
//       `Formatter` or the raw `Write` trait.
impl Display for Bijective {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        let margin = ((self.inner.len() as f64).log10() as usize) + 1;
        let width = textwrap::termwidth(); // TODO: Use `console` for this.
        let width = cmp::max(width - cmp::min(width, margin + 6), 16);
        for (n, (source, destination)) in self.inner.iter().enumerate() {
            let source = source.to_string_lossy();
            let mut lines = textwrap::wrap(source.as_ref(), width).into_iter();
            write!(
                formatter,
                "{:0>width$} ─┬── {}\n",
                n,
                lines.next().unwrap(),
                width = margin
            )?;
            for line in lines {
                write!(
                    formatter,
                    "{: >width$}   {}\n",
                    "│",
                    line,
                    width = margin + 3
                )?;
            }

            let destination = destination.to_string_lossy();
            let mut lines = textwrap::wrap(destination.as_ref(), width).into_iter();
            write!(
                formatter,
                "{: >width$} {}\n",
                "╰─❯",
                lines.next().unwrap(),
                width = margin + 5
            )?;
            for line in lines {
                write!(
                    formatter,
                    "{}{}\n",
                    iter::repeat(" ").take(margin + 6).collect::<String>(),
                    line,
                )?;
            }
        }
        Ok(())
    }
}

impl IntoIterator for Bijective {
    type Item = <BiMap<PathBuf, PathBuf> as IntoIterator>::Item;
    type IntoIter = <BiMap<PathBuf, PathBuf> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl Manifest for Bijective {
    fn insert(
        &mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<PathBuf>,
    ) -> io::Result<()> {
        self.inner
            .insert_no_overwrite(source.into(), destination.into())
            .map_err(|_| Error::from(ErrorKind::Other))
    }
}

#[derive(Clone, Debug)]
pub struct Transform<'t> {
    pub from: FromPattern,
    pub to: ToPattern<'t>,
}

impl<'t> Transform<'t> {
    pub fn read<M>(&self, directory: impl AsRef<Path>, depth: usize) -> io::Result<M>
    where
        M: Manifest,
    {
        let mut manifest = M::default();
        for entry in WalkDir::new(directory.as_ref().normalize()?)
            .follow_links(false)
            .min_depth(1)
            .max_depth(depth)
        {
            let entry = entry?;
            if entry.file_type().is_file() {
                if let Some(find) = entry
                    .path()
                    .file_name()
                    .and_then(|name| self.from.find(name.to_str().unwrap()))
                {
                    let source = entry.path();
                    let mut destination = source.to_path_buf();
                    destination.pop();
                    destination.push(self.to.join(&find).unwrap()); // TODO:
                    manifest.insert(source, destination)?;
                }
            }
        }
        Ok(manifest)
    }
}
