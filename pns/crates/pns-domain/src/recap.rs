//! What the recap says, composed from what the window held.
//!
//! POLICY ONLY: no file, no clock, no environment. The composition root reads
//! the ring, runs the summarizer and prints; this decides what the body says.

pub mod activity;
pub mod agent;
pub mod budget;
pub mod document;
pub mod external;
pub mod git_block;
pub mod night;
mod options;
pub mod prompt;
pub mod sanitize;
pub mod sections;
pub mod summarizer;
pub mod window;
pub use options::{Recap, SECTION_NAMES, Sources};

#[cfg(test)]
mod tests {
    mod agent;
    mod answers;
    mod composition;
    mod external;
    mod external_lines;
    mod fixtures;
    mod git_block;
}
