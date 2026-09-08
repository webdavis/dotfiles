use crate::KnownGoodTuple;

mod bounds;
mod file;

pub use bounds::AuditBounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditKind {
    Content,
    Mode,
    Owner,
    Missing,
    Irregular,
    Oversize,
    Unreadable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditRefusal {
    Missing,
    Unavailable,
    Untrustworthy,
    Malformed,
    Overlong,
    Budget,
}

#[derive(Debug, Clone, Copy)]
pub enum AuditFile<'a> {
    Missing,
    Symlink,
    Irregular,
    Regular {
        size: Option<u64>,
        mode: Option<&'a str>,
        uid: Option<&'a str>,
        digest: Option<&'a str>,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct AuditRow<'a> {
    pub line: &'a str,
    pub checked_at: u64,
    pub file: AuditFile<'a>,
}

#[derive(Debug, Clone, Copy)]
pub enum AuditManifest<'a> {
    Missing,
    Untrustworthy,
    Rows(&'a [AuditRow<'a>]),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditFinding<'a> {
    pub kind: AuditKind,
    pub path: &'a str,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct AuditReport<'a> {
    pub findings: Vec<AuditFinding<'a>>,
    pub refusal: Option<AuditRefusal>,
}

impl AuditReport<'_> {
    pub fn text(&self) -> String {
        let mut text = String::new();
        for finding in &self.findings {
            text.push_str(finding.kind.word());
            text.push(' ');
            text.push_str(finding.path);
            text.push('\n');
        }
        if let Some(reason) = self.refusal {
            text.push_str(reason.word());
            text.push('\n');
        }
        text
    }
}

pub fn audit_scan<'a>(
    manifests: Option<[AuditManifest<'a>; 2]>,
    bounds: AuditBounds,
    started_at: u64,
) -> AuditReport<'a> {
    let mut report = AuditReport::default();
    let deadline = started_at.saturating_add(bounds.seconds);
    let Some(manifests) = manifests else {
        report.refusal = Some(AuditRefusal::Unavailable);
        return report;
    };
    for manifest in manifests {
        let rows = match manifest {
            AuditManifest::Rows(rows) if !rows.is_empty() => rows,
            AuditManifest::Rows(_) | AuditManifest::Missing => {
                report.refusal = Some(AuditRefusal::Missing);
                return report;
            }
            AuditManifest::Untrustworthy => {
                report.refusal = Some(AuditRefusal::Untrustworthy);
                return report;
            }
        };
        for (index, row) in rows.iter().enumerate() {
            // The entry ceiling is per manifest. The caller supplies the start
            // sampled before inspection; both manifests share the same deadline.
            let refusal = if index >= bounds.entries as usize {
                Some(AuditRefusal::Overlong)
            } else if row.checked_at >= deadline {
                Some(AuditRefusal::Budget)
            } else {
                None
            };
            if refusal.is_some() {
                report.refusal = refusal;
                return report;
            }
            let Some(want) = KnownGoodTuple::parse_line(row.line) else {
                report.refusal = Some(AuditRefusal::Malformed);
                return report;
            };
            report
                .findings
                .extend(
                    file::kinds(want, row.file, bounds.bytes)
                        .into_iter()
                        .map(|kind| AuditFinding {
                            kind,
                            path: want.path,
                        }),
                );
        }
    }
    // Bash streams findings before a refusal. Keeping them is part of the
    // contract: mixed output becomes an unknown audit condition, never clean.
    report
}

impl AuditKind {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Mode => "mode",
            Self::Owner => "owner",
            Self::Missing => "missing",
            Self::Irregular => "irregular",
            Self::Oversize => "oversize",
            Self::Unreadable => "unreadable",
        }
    }
}

impl AuditRefusal {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unavailable => "unavailable",
            Self::Untrustworthy => "untrustworthy",
            Self::Malformed => "malformed",
            Self::Overlong => "overlong",
            Self::Budget => "budget",
        }
    }
}

#[cfg(test)]
mod tests;
