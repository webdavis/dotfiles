//! What one lane did, as the record and the alert read it.

/// What one lane did: how many things went wrong, whether it DEFERRED instead
/// of running, the lines the record carries about it, and the last of those
/// lines that reported a FAILURE.
///
/// DEFERRED IS NOT A FAILURE. A lane that exited `DEFERRED_EXIT_CODE` did not
/// run at all; that is a fact worth a distinct line in the record, and never
/// a reason to alert or to count toward `failures`.
///
/// THE LAST FAILURE IS KEPT SEPARATELY because the lane continues past one,
/// so the last line written is routinely a later success. The alert has room
/// for one sentence and it has to be the one naming what to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneReport {
    pub name: String,
    pub failures: usize,
    pub deferred: bool,
    pub lines: Vec<String>,
    pub last_failure: Option<String>,
}

impl LaneReport {
    /// A report for a lane that has not done anything yet.
    pub fn new(name: &str) -> Self {
        LaneReport {
            name: name.to_string(),
            failures: 0,
            deferred: false,
            lines: Vec::new(),
            last_failure: None,
        }
    }

    /// One thing that went WRONG: counted, recorded and remembered, in one
    /// place. A lane cannot count a failure it did not also make alertable,
    /// which is the drift a second `failures += 1` beside a bare push invites.
    pub fn failed(&mut self, line: String) {
        self.failures += 1;
        self.last_failure = Some(line.clone());
        self.lines.push(line);
    }

    /// The lane DEFERRED: nothing was attempted, so this is recorded rather
    /// than counted as a failure and never fires the per-run alert. Distinct
    /// from `failed`, which the caller must never also call for the same
    /// verdict: a lane either deferred or it did not.
    pub fn deferred(&mut self, line: String) {
        self.deferred = true;
        self.lines.push(line);
    }

    /// One thing that went right, or a fact the record carries.
    pub fn noted(&mut self, line: String) {
        self.lines.push(line);
    }
}
