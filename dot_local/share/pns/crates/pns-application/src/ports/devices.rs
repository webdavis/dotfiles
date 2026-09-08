//! The two appliances on the network a use case asks questions of.

/// The router's complete, decoded client listing. A missing or malformed
/// response is unknown, while a parsed empty listing is evidence of absence.
pub trait Router {
    fn clients(&self) -> Option<Vec<pns_domain::home::Client>>;
}

/// The staleness episode this machine last reported. Writes are best effort;
/// a failed write may repeat a warning but cannot change a home verdict.
pub trait StalenessMemory {
    fn remembered(&self) -> Option<String>;
    fn remember(&self, episode: Option<&str>);
}
