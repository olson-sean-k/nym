use miette::Diagnostic;
use std::path::Path;
use thiserror::Error;

use crate::actuator::{Actuation, Operation};
use crate::manifest::{Manifest, ManifestError};
use crate::pattern::{FromPattern, FromPatternError, ToPattern, ToPatternError};
use crate::policy::{self, Policy, PolicyError};

#[derive(Debug, Diagnostic, Error)]
#[diagnostic(transparent)]
#[error("failed to build manifest")]
pub struct TransformError {
    #[source]
    kind: ErrorKind,
}

impl From<FromPatternError> for TransformError {
    fn from(error: FromPatternError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<ManifestError> for TransformError {
    fn from(error: ManifestError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<PolicyError> for TransformError {
    fn from(error: PolicyError) -> Self {
        TransformError { kind: error.into() }
    }
}

impl From<ToPatternError> for TransformError {
    fn from(error: ToPatternError) -> Self {
        TransformError { kind: error.into() }
    }
}

#[derive(Debug, Diagnostic, Error)]
enum ErrorKind {
    #[diagnostic(transparent)]
    #[error(transparent)]
    FromPattern(#[from] FromPatternError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    Policy(#[from] PolicyError),
    #[diagnostic(transparent)]
    #[error(transparent)]
    ToPattern(#[from] ToPatternError),
}

#[derive(Clone, Debug)]
pub struct Transform<'p> {
    policy: Policy,
    from: FromPattern<'p>,
    to: ToPattern<'p>,
}

impl<'p> Transform<'p> {
    pub fn new(policy: Policy, from: FromPattern<'p>, to: ToPattern<'p>) -> Self {
        Transform { policy, from, to }
    }

    pub fn read<W>(
        self,
        directory: impl AsRef<Path>,
        depth: usize,
    ) -> Result<Actuation<W>, TransformError>
    where
        W: Operation,
    {
        let Transform { policy, from, to } = self;
        let mut manifest = Manifest::default();
        for entry in from.walk(directory.as_ref(), depth) {
            let entry = entry?;
            let source = entry.path();
            let mut destination = directory.as_ref().to_path_buf();
            destination.push(to.resolve(&source, entry.matched())?);
            policy::check(&policy, source, &destination)?;
            manifest.insert(source, destination)?;
        }
        Ok(Actuation::new(policy, manifest))
    }
}
