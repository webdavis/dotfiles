//! The one GitHub command-line interface the recap reads, in one place.
//!
//! ONE TOOL, ONE SEAM. Both recap sections that ask GitHub a question (the
//! merged listing and the Git block's pull-request lookup) come through here,
//! so "which GitHub CLI does pns depend on" is a single answer rather than a
//! constant in each adapter that a later edit could move on one side only.
//!
//! `gh` CARRIES ITS OWN AUTH AND THIS NEVER TOUCHES IT. No token is read, no
//! credential is passed, and every spawn below is a LIST, which is what bounds
//! a remote answer becoming this machine's problem.
//!
//! RESOLVED THROUGH PATH, like `herdr` and unlike the system binaries: it is
//! installed wherever this machine's package manager put it, and a context
//! whose PATH does not carry it reads as unavailable, which costs the calling
//! section and nothing else.

use crate::run_bounded;
use std::{process::Command, time::Duration};

/// One `gh` listing as the text it printed, or None when `gh` is not on PATH,
/// refused, outlasted the deadline or ran past `read_max`.
///
/// A TRUNCATED READ IS NOT JSON, so the caps fail CLOSED into "unavailable"
/// rather than into half a section. `read_max` is the caller's, because the two
/// listings differ by orders of magnitude in what they can honestly return.
pub(super) fn listing(arguments: &[&str], cwd: Option<&str>, read_max: u64) -> Option<String> {
    let mut command = Command::new(GH);
    command.args(arguments);
    if let Some(cwd) = cwd.filter(|cwd| !cwd.is_empty()) {
        command.current_dir(cwd);
    }
    run_bounded(command, None, DEADLINE, read_max)
}

/// The listing tool, resolved through PATH.
const GH: &str = "gh";

/// How long a listing may take. THIRTY SECONDS, thirty times the second both
/// calls MEASURED and short of anything a person would call working. Nobody is
/// waiting on it, so this exists to stop a wedged network call holding the
/// whole recap rather than to hurry a slow one.
const DEADLINE: Duration = Duration::from_secs(30);
