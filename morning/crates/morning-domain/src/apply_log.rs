//! The chezmoi apply transcript, reduced to the two things a morning needs:
//! did the last apply pass, and when.

/// The outcome of one apply.
#[derive(Debug, PartialEq, Eq)]
pub struct Apply {
    /// The transcript's own `result:` word, `OK` or `FAILED`.
    pub result: String,
    /// The `finished:` timestamp, absent when the apply never finished.
    pub finished: Option<String>,
}

impl Apply {
    /// The one line the brief prints for this apply.
    pub fn summary(&self) -> String {
        match &self.finished {
            Some(when) => format!("{} at {when}", self.result),
            None => format!("{}, no finish time recorded", self.result),
        }
    }
}

/// Reads an apply transcript. `None` when it carries no `result:` line, which
/// is what an apply killed mid-run leaves behind.
pub fn parse(transcript: &str) -> Option<Apply> {
    let result = field(transcript, "result")?;
    Some(Apply {
        result,
        finished: field(transcript, "finished"),
    })
}

/// The value of the LAST `<name>:` line, so a transcript holding more than one
/// apply reports the most recent.
fn field(transcript: &str, name: &str) -> Option<String> {
    transcript
        .lines()
        .filter_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
        .map(|value| value.trim().to_string())
        .rfind(|value| !value.is_empty())
}

#[cfg(test)]
mod tests;
