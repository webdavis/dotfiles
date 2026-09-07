use crate::Marker;

/// The header and previous marker sampled before a run's lane loop. Every
/// lane receives the same facts rather than resampling them during execution.
pub struct RunFacts<'a> {
    pub host: &'a str,
    pub started_epoch: i64,
    pub started_iso: &'a str,
    pub marker: &'a Marker,
}
