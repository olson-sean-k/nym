use regex::bytes;

pub enum Selector<'t> {
    ByIndex(usize),
    ByName(&'t str),
}

#[derive(Debug)]
enum MaybeOwnedCaptures<'t> {
    Borrowed(bytes::Captures<'t>),
    Owned(ClonedCaptures),
}

impl<'t> MaybeOwnedCaptures<'t> {
    fn into_owned(self) -> MaybeOwnedCaptures<'static> {
        match self {
            MaybeOwnedCaptures::Borrowed(borrowed) => ClonedCaptures::from(borrowed).into(),
            MaybeOwnedCaptures::Owned(owned) => owned.into(),
        }
    }
}

impl<'t> From<bytes::Captures<'t>> for MaybeOwnedCaptures<'t> {
    fn from(captures: bytes::Captures<'t>) -> Self {
        MaybeOwnedCaptures::Borrowed(captures)
    }
}

impl From<ClonedCaptures> for MaybeOwnedCaptures<'static> {
    fn from(captures: ClonedCaptures) -> Self {
        MaybeOwnedCaptures::Owned(captures)
    }
}

#[derive(Debug)]
struct ClonedCaptures {
    matched: Vec<u8>,
    ranges: Vec<Option<(usize, usize)>>,
}

impl ClonedCaptures {
    pub fn get(&self, selector: Selector<'_>) -> Option<&[u8]> {
        match selector {
            Selector::ByIndex(index) => {
                if index == 0 {
                    Some(self.matched.as_ref())
                }
                else {
                    self.ranges
                        .get(index - 1)
                        .map(|range| range.map(|range| &self.matched[range.0..range.1]))
                        .flatten()
                }
            }
            Selector::ByName(_) => todo!(),
        }
    }
}

impl<'t> From<bytes::Captures<'t>> for ClonedCaptures {
    fn from(captures: bytes::Captures<'t>) -> Self {
        let matched = captures.get(0).unwrap().as_bytes().into();
        let ranges = captures
            .iter()
            .skip(1)
            .map(|capture| capture.map(|capture| (capture.start(), capture.end())))
            .collect();
        ClonedCaptures { matched, ranges }
    }
}

#[derive(Debug)]
pub struct Captures<'t> {
    inner: MaybeOwnedCaptures<'t>,
}

impl<'t> Captures<'t> {
    pub fn into_owned(self) -> Captures<'static> {
        let Captures { inner } = self;
        Captures {
            inner: inner.into_owned(),
        }
    }

    pub fn matched(&self) -> &[u8] {
        self.get(Selector::ByIndex(0)).unwrap()
    }

    pub fn get(&self, selector: Selector<'_>) -> Option<&[u8]> {
        match self.inner {
            MaybeOwnedCaptures::Borrowed(ref captures) => match selector {
                Selector::ByIndex(index) => captures.get(index),
                Selector::ByName(name) => captures.name(name),
            }
            .map(|capture| capture.as_bytes()),
            MaybeOwnedCaptures::Owned(ref captures) => captures.get(selector),
        }
    }
}

// TODO: Maybe this shouldn't be part of the public API.
impl<'t> From<bytes::Captures<'t>> for Captures<'t> {
    fn from(captures: bytes::Captures<'t>) -> Self {
        Captures {
            inner: captures.into(),
        }
    }
}

impl From<ClonedCaptures> for Captures<'static> {
    fn from(captures: ClonedCaptures) -> Self {
        Captures {
            inner: captures.into(),
        }
    }
}
