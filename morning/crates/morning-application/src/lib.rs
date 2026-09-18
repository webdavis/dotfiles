//! How a brief is assembled: every source read at once, each one bounded, and
//! a section for every source whatever happened to it.
//!
//! The reading itself is a closure the composition root hands in, so this
//! crate knows nothing about files, processes or clocks.

use morning_domain::{Section, SectionBody};

/// One titled source of the brief.
pub struct Source<'a> {
    pub title: &'a str,
    pub read: Box<dyn Fn() -> SectionBody + Send + Sync + 'a>,
}

impl<'a> Source<'a> {
    pub fn new(title: &'a str, read: impl Fn() -> SectionBody + Send + Sync + 'a) -> Self {
        Self {
            title,
            read: Box::new(read),
        }
    }
}

/// Reads every source CONCURRENTLY and returns one section per source, in the
/// order the sources were given.
///
/// Concurrent because the page is worth nothing if it arrives late: a
/// sequential read costs the SUM of the sources' waits, and one unauthenticated
/// `gh` would hold the whole morning behind it. Each closure bounds itself, so
/// the page costs the SLOWEST source rather than all of them. A panicking
/// source becomes an unavailable section rather than taking the run down.
pub fn gather(sources: Vec<Source<'_>>) -> Vec<Section> {
    std::thread::scope(|scope| {
        let readers: Vec<_> = sources
            .iter()
            .map(|source| scope.spawn(|| (source.read)()))
            .collect();
        sources
            .iter()
            .zip(readers)
            .map(|(source, reader)| Section {
                title: source.title.to_string(),
                body: reader.join().unwrap_or_else(|_| {
                    SectionBody::Unavailable("the reader panicked".to_string())
                }),
            })
            .collect()
    })
}

#[cfg(test)]
mod tests;
