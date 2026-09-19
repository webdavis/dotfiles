//! What a morning brief IS, with no idea where any of it came from.
//!
//! Every function here is pure text in, text out: an apply transcript, a
//! ledger, a list of sections. The reading, spawning and timing live in
//! `morning-adapters`, which is what keeps the parsing and the page testable
//! without a filesystem, a network or a clock.

pub mod apply_log;
pub mod ledger;
pub mod page;

pub use apply_log::Apply;
pub use ledger::Ledger;
pub use page::{Line, Section, SectionBody, render};
