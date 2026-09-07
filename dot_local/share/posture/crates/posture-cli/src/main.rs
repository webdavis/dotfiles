//! The `posture` binary: argv in, exit code out.
//!
//! Every word is refused until its subcommand lands (spec S298, S341): usage
//! goes to stderr, nothing to stdout, and the exit status is 2. The words
//! listed are the specification's section 1 table, one per entry point, so a
//! caller repointed here early fails loudly rather than silently doing
//! nothing.

use std::io::{self, Write};
use std::process::ExitCode;

const USAGE: &str = "usage: posture <subcommand> [args]
  alert | poll | funnel | watchdog | digest | heartbeat | converge
  allowlist add <label> | allowlist deny <label> | allowlist list
  enrich <path>
  ssh install|verify|reload|rollback|print-config|print-path
no subcommand is implemented yet; every word exits 2
";

fn main() -> ExitCode {
    if io::stderr().lock().write_all(USAGE.as_bytes()).is_err() {
        return ExitCode::from(2);
    }
    ExitCode::from(2)
}
