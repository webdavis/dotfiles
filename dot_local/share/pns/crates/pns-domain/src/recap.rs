//! What the recap says, composed from what the window held.
//!
//! POLICY ONLY: no file, no clock, no environment. The composition root reads
//! the ring, runs the summarizer and prints; this decides what the body says.

pub mod budget;
pub mod external;
pub mod night;
pub mod prompt;
pub mod sanitize;
pub mod sections;

#[cfg(test)]
mod tests {
    mod answers;
    mod composition;
    mod external;
    mod external_lines;
    mod fixtures;
}
