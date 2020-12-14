use normpath::PathExt as _;
use std::convert::TryFrom;
use std::io;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};

pub trait PathExt {
    #[inline(always)]
    fn is_verbatim(&self) -> bool {
        false
    }
}

impl PathExt for Path {
    #[cfg(target_os = "windows")]
    fn is_verbatim(&self) -> bool {
        if let Some(Component::Prefix(prefix)) = self.components().next() {
            prefix.kind().is_verbatim()
        }
        else {
            false
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialOrd, PartialEq)]
pub struct CanonicalPath {
    inner: PathBuf,
}

impl AsRef<Path> for CanonicalPath {
    fn as_ref(&self) -> &Path {
        self.inner.as_ref()
    }
}

impl Deref for CanonicalPath {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Into<PathBuf> for CanonicalPath {
    fn into(self) -> PathBuf {
        self.inner
    }
}

impl<'p> TryFrom<&'p Path> for CanonicalPath {
    type Error = io::Error;

    fn try_from(path: &'p Path) -> Result<Self, Self::Error> {
        let inner = if path.is_file() {
            if let Some(parent) = path.parent() {
                let parent = parent.normalize()?.into_path_buf();
                parent.join(path.file_name().unwrap())
            }
            else {
                path.to_path_buf()
            }
        }
        else {
            path.normalize()?.into_path_buf()
        };
        Ok(CanonicalPath { inner })
    }
}

impl TryFrom<PathBuf> for CanonicalPath {
    type Error = io::Error;

    fn try_from(path: PathBuf) -> Result<Self, Self::Error> {
        Self::try_from(path.as_ref())
    }
}
