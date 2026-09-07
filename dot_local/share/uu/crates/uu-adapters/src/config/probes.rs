//! The three questions every config test puts to the parser: what this text
//! parsed into, why it was refused, and which adapter a lane got.
//!
//! `#[cfg(test)]` at the parent, so none of this enters a production build.

use super::{Config, LaneKind, parse_config};

pub(crate) fn parsed(text: &str) -> Config {
    parse_config(text).expect("this config is valid")
}

pub(crate) fn refusal(text: &str) -> String {
    match parse_config(text) {
        Err(error) => error.detail().to_string(),
        Ok(config) => panic!("this config should have been refused, got {config:?}"),
    }
}

/// One lane's ADAPTER. Every assertion that uses this is about what a block
/// parsed into, never about the deadline beside it, which `deadline.rs` owns.
pub(crate) fn kind<'a>(config: &'a Config, name: &str) -> Option<&'a LaneKind> {
    config.lanes.get(name).map(|lane| &lane.kind)
}
