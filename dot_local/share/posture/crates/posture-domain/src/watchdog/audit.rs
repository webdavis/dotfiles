mod problem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFingerprint(String);
impl AuditFingerprint {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .then(|| Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuditMemory {
    pub fingerprint: Option<AuditFingerprint>,
    pub streak: u8,
    pub paged: Option<AuditFingerprint>,
}
impl AuditMemory {
    pub fn from_readings(fingerprint: &str, streak: &str, paged: &str) -> Self {
        let streak = if !streak.is_empty()
            && streak.len() <= 3
            && streak.bytes().all(|b| b.is_ascii_digit())
        {
            streak.parse::<u16>().unwrap_or(0).min(99) as u8
        } else {
            0
        };
        Self {
            fingerprint: AuditFingerprint::parse(fingerprint),
            streak,
            paged: AuditFingerprint::parse(paged),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct AuditJudgment {
    pub next: AuditMemory,
    pub problem: Option<String>,
}
// Command substitution drops trailing newlines before the old sort/hash pipeline.
// Sorting bytewise retains duplicate columns and excludes report order from identity.
// The adapter owns hashing these bytes, never the domain or a persisted raw path.
pub fn audit_fingerprint_input(report: &str) -> String {
    let mut lines: Vec<_> = report.trim_end_matches('\n').split('\n').collect();
    lines.sort_unstable();
    lines.join("\n") + "\n"
}
pub fn judge_audit(
    completed: bool,
    report: &str,
    fingerprint: Option<AuditFingerprint>,
    previous: &AuditMemory,
) -> AuditJudgment {
    let report = report.trim_end_matches('\n');
    if completed && report.is_empty() {
        return AuditJudgment {
            next: AuditMemory::default(),
            problem: None,
        };
    }
    let Some(fingerprint) = fingerprint else {
        return AuditJudgment {
            next: AuditMemory::default(),
            problem: Some(problem::text(completed, report)),
        };
    };
    let streak = if previous.fingerprint.as_ref() == Some(&fingerprint) {
        previous.streak.saturating_add(1).min(99)
    } else {
        1
    };
    let page = streak >= 2 && previous.paged.as_ref() != Some(&fingerprint);
    let paged = if page {
        Some(fingerprint.clone())
    } else {
        previous.paged.clone()
    };
    // This is proposed state only. The use case must not persist it until a page
    // is durably accepted; failure must leave the previous state available to retry.
    AuditJudgment {
        next: AuditMemory {
            fingerprint: Some(fingerprint),
            streak,
            paged,
        },
        problem: page.then(|| problem::text(completed, report)),
    }
}
#[cfg(test)]
mod tests;
