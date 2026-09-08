use crate::AuditKind;

pub(super) fn text(completed: bool, report: &str) -> String {
    if !completed {
        return refusal(report).into();
    }
    let count = report.split('\n').count();
    if !(1..=999_999).contains(&count) {
        return refusal("unknown").into();
    }
    let labels: Vec<_> = [
        (AuditKind::Content, "content changed"),
        (AuditKind::Mode, "permissions changed"),
        (AuditKind::Owner, "ownership changed"),
        (AuditKind::Missing, "file missing"),
        (AuditKind::Irregular, "not a regular file"),
        (AuditKind::Oversize, "too large to hash"),
        (AuditKind::Unreadable, "unreadable"),
    ]
    .into_iter()
    .filter_map(|(kind, label)| {
        let prefix = format!("{} ", kind.word());
        report
            .split('\n')
            .any(|line| line.starts_with(&prefix))
            .then_some(label)
    })
    .collect();
    let kinds = if labels.is_empty() {
        String::new()
    } else {
        format!(" ({})", labels.join(", "))
    };
    format!(
        "{count} divergence(s) from a known-good manifest{kinds}; no file event reported this, which is what a hard-linked or relocated script, or a chmod through such an alias, looks like"
    )
}
fn refusal(token: &str) -> &'static str {
    // Matching only these literals also rejects mixed findings/refusal output.
    // Unknown words and hostile bytes must never reach the page.
    match token {
        "missing" => {
            "a known-good manifest is missing or unreadable, so the periodic manifest audit cannot verify the files it covers; tampering would go unseen until it is restored"
        }
        "unavailable" => {
            "the periodic manifest audit is not installed completely (a helper it needs is missing), so the deployed files are unverified against their known-good manifests"
        }
        "untrustworthy" => {
            "a known-good manifest is no longer root-owned (or is group/world-writable), so it can no longer vouch for the files it covers"
        }
        "malformed" => {
            "a known-good manifest holds a malformed entry, so the periodic manifest audit cannot verify the files it covers"
        }
        "overlong" => {
            "a known-good manifest lists more files than one audit tick will examine, so the files it covers cannot be fully verified"
        }
        "budget" => {
            "the periodic manifest audit ran out of its time budget before checking every manifested file, so the files it covers cannot be fully verified"
        }
        _ => {
            "the periodic manifest audit could not verify the deployed files against their known-good manifests"
        }
    }
}
