//! Every expectation here was captured by RUNNING the jq renderer this replaces,
//! not read off its source. The two clauses the specification records as
//! unpinned, the file-event action and the signing verdict, were captured the
//! same way and now have the tests they lacked.

use super::*;

mod caps;
mod fields;
mod golden;
mod headers;

/// A critical finding of `query`, with nothing else set.
pub(super) fn critical(query: &str) -> PageFinding<'_> {
    PageFinding {
        query,
        severity: Severity::Critical,
        enrichment_path: "",
        columns: PageColumns::default(),
        act: None,
        signing: None,
        triage: None,
    }
}

/// The rendered body of a single finding.
pub(super) fn body(finding: PageFinding<'_>) -> String {
    render_page(&[finding]).body
}

#[test]
fn a_crit_finding_renders_a_plain_english_header_its_decision_fields_and_a_next_step() {
    let mut finding = critical("persistence_launchd");
    finding.enrichment_path = "/tmp/p";
    finding.columns.label = Some("com.x");
    finding.columns.program = Some("/bin/x");
    finding.signing = Some(Signing {
        untrusted: false,
        text: "signed: Apple",
    });
    assert_eq!(
        body(finding),
        "**New startup item**\n\
         - **What:** `com.x`\n\
         - **Program:** `/bin/x`\n\
         - **Signing:** signed: Apple\n\
         - Did you set this up? If not, it **auto-runs at every login** - likely malware.\n\
         - **Inspect:** `cat -- '/tmp/p'`"
    );
}

#[test]
fn an_untrusted_signing_verdict_is_bolded_behind_a_warning_glyph() {
    let mut finding = critical("persistence_launchd");
    finding.columns.label = Some("com.x");
    finding.columns.program = Some("/bin/x");
    finding.signing = Some(Signing {
        untrusted: true,
        text: "unsigned",
    });
    assert!(body(finding).contains("- **Signing:** ⚠️ **unsigned**"));
}

#[test]
fn only_critical_findings_reach_the_page() {
    let notice = PageFinding {
        severity: Severity::Notice,
        ..critical("persistence_launchd")
    };
    let info = PageFinding {
        severity: Severity::Info,
        ..critical("new_admin_user")
    };
    let page = render_page(&[notice, info]);
    assert_eq!(page.count, 0);
    assert_eq!(page.body, "");
}
