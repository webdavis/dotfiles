//! What one lane did, as the record and the alert read it.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneVerdict {
    Completed,
    Pending,
    Deferred,
    Failed,
}

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
    failures: usize,
    verdict: LaneVerdict,
    pub lines: Vec<String>,
    last_failure: Option<String>,
}

impl LaneReport {
    /// A report for a lane that has not done anything yet.
    pub fn new(name: &str) -> Self {
        LaneReport {
            name: name.to_string(),
            failures: 0,
            verdict: LaneVerdict::Completed,
            lines: Vec::new(),
            last_failure: None,
        }
    }

    /// One thing that went WRONG: counted, recorded and remembered, in one
    /// place. A lane cannot count a failure it did not also make alertable,
    /// which is the drift a second `failures += 1` beside a bare push invites.
    pub fn failed(&mut self, line: String) {
        self.failures += 1;
        self.verdict = LaneVerdict::Failed;
        self.last_failure = Some(line.clone());
        self.lines.push(line);
    }

    /// The lane DEFERRED: nothing was attempted, so this is recorded rather
    /// than counted as a failure and never fires the per-run alert. Distinct
    /// from `failed`; an earlier failure retains precedence over a deferral.
    pub fn deferred(&mut self, line: String) {
        if self.verdict != LaneVerdict::Failed {
            self.verdict = LaneVerdict::Deferred;
        }
        self.lines.push(line);
    }

    pub fn pending(&mut self, line: String) {
        if matches!(self.verdict, LaneVerdict::Completed | LaneVerdict::Pending) {
            self.verdict = LaneVerdict::Pending;
        }
        self.lines.push(line);
    }

    pub fn verdict(&self) -> LaneVerdict {
        self.verdict
    }

    pub fn failures(&self) -> usize {
        self.failures
    }

    pub fn last_failure(&self) -> Option<&str> {
        self.last_failure.as_deref()
    }

    pub fn succeeded(&self) -> bool {
        matches!(self.verdict, LaneVerdict::Completed | LaneVerdict::Pending)
    }

    /// One thing that went right, or a fact the record carries.
    pub fn noted(&mut self, line: String) {
        self.lines.push(line);
    }
}

#[cfg(test)]
mod tests;
