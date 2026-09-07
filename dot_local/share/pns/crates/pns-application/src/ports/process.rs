//! Running another program.

/// Runs a command and returns its stdout, or `None` when it cannot be run or
/// exits non-zero. The seam every probe reads the world through.
///
/// `None` covers every way of getting no answer: a blown deadline, a non-zero
/// exit, output past the cap. No caller acts differently on them, and every
/// reading behind this port states its own fail direction, all of which read
/// no answer as unknown. The deadline and the cap are the adapter's, not this
/// declaration's. Statements: S089.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;
}
