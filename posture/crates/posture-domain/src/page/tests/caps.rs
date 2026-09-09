//! The three bounds that keep a page deliverable, and the quoting that keeps a
//! pasted next step from executing a path.

use super::{body, critical};
use crate::page::{BLOCK_LIMIT, BODY_LIMIT, PageFinding, render_page};

#[test]
fn a_field_value_over_240_characters_is_truncated_behind_a_marker() {
    let long = "y".repeat(250);
    let mut finding = critical("persistence_launchd");
    finding.columns.program = Some(&long);
    let expected = format!("- **Program:** `{}…(truncated)`", "y".repeat(240));
    assert!(body(finding).contains(&expected));
}

#[test]
fn a_field_value_at_exactly_240_characters_is_left_alone() {
    let exact = "y".repeat(240);
    let mut finding = critical("persistence_launchd");
    finding.columns.program = Some(&exact);
    let rendered = body(finding);
    assert!(rendered.contains(&format!("- **Program:** `{exact}`")));
    assert!(!rendered.contains("truncated"));
}

#[test]
fn a_field_is_cut_by_characters_so_a_multi_byte_one_is_never_split() {
    let long = "é".repeat(250);
    let mut finding = critical("persistence_launchd");
    finding.columns.program = Some(&long);
    assert!(body(finding).contains(&format!("`{}…(truncated)`", "é".repeat(240))));
}

#[test]
fn the_page_renders_at_most_eight_blocks_and_counts_every_crit_finding_it_dropped() {
    let findings: Vec<PageFinding<'_>> = (0..11).map(|_| critical("new_admin_user")).collect();
    let page = render_page(&findings);
    assert_eq!(
        page.count, 11,
        "the count must include what the cap dropped"
    );
    assert_eq!(
        page.body.matches("**New administrator account**").count(),
        BLOCK_LIMIT
    );
    assert!(
        page.body
            .ends_with("… and 3 more CRITICAL finding(s) - see results.log"),
        "{}",
        page.body
    );
}

#[test]
fn exactly_eight_blocks_render_without_a_dropped_marker() {
    let findings: Vec<PageFinding<'_>> = (0..8).map(|_| critical("new_admin_user")).collect();
    let page = render_page(&findings);
    assert_eq!(page.count, 8);
    assert!(!page.body.contains("more CRITICAL"));
}

#[test]
fn the_page_body_is_hard_capped_below_the_2000_char_delivery_limit() {
    // EIGHT BLOCKS IS A COUNT, NOT A LENGTH. Each of these carries a field at
    // its own cap, so the block cap alone leaves the body far over the limit.
    let long = "y".repeat(240);
    let findings: Vec<PageFinding<'_>> = (0..8)
        .map(|_| {
            let mut finding = critical("persistence_launchd");
            finding.columns.label = Some(&long);
            finding.columns.program = Some(&long);
            finding
        })
        .collect();
    let page = render_page(&findings);
    let body = page.body;
    assert!(
        body.chars().count() > BODY_LIMIT,
        "the truncated body keeps its own marker, so it exceeds the cut point"
    );
    assert!(
        body.ends_with("\n… (truncated to fit the 2000-char limit - see results.log)"),
        "{body}"
    );
    assert!(
        body.chars().count() < 2000,
        "the whole body must fit the delivery limit: {}",
        body.chars().count()
    );
}

#[test]
fn a_quote_breaking_path_never_executes_in_a_codesign_next_step_command() {
    let mut finding = critical("suid_bin_unexpected");
    finding.enrichment_path = "/tmp/a b'; rm -rf /";
    assert!(
        body(finding).contains(r"- **Inspect:** `codesign -dv -- '/tmp/a b'\''; rm -rf /'`"),
        "{}",
        body(finding)
    );
}

#[test]
fn a_quote_breaking_path_never_executes_in_a_cat_sudo_cat_or_shasum_next_step_command() {
    let hostile = "/tmp/a b'; rm -rf /";

    let mut launchd = critical("persistence_launchd");
    launchd.enrichment_path = hostile;
    assert!(body(launchd).contains(r"`cat -- '/tmp/a b'\''; rm -rf /'`"));

    let mut watched = critical("file_events_recent");
    watched.enrichment_path = hostile;
    watched.columns.category = Some("sudoers");
    assert!(body(watched).contains(r"`sudo cat -- '/tmp/a b'\''; rm -rf /'`"));

    let mut ours = critical("file_events_recent");
    ours.enrichment_path = hostile;
    ours.columns.category = Some("pipeline_integrity");
    assert!(body(ours).contains(r"`shasum -a 256 -- '/tmp/a b'\''; rm -rf /'`"));
}

#[test]
fn a_command_substitution_path_never_executes_in_a_rendered_next_step_command() {
    let mut finding = critical("persistence_launchd");
    finding.enrichment_path = "/tmp/$(rm -rf /)/`whoami`";
    let rendered = body(finding);
    // Inside single quotes both forms are literal text; the backticks are also
    // stripped, because they would end the code span the line renders in.
    assert!(
        rendered.contains("`cat -- '/tmp/$(rm -rf /)/whoami'`"),
        "{rendered}"
    );
}

#[test]
fn a_finding_with_no_enrichment_path_offers_no_review_line() {
    let rendered = body(critical("recent_logins"));
    assert!(!rendered.contains("Review"), "{rendered}");
}

#[test]
fn an_unmapped_finding_with_a_path_offers_it_for_review() {
    let mut finding = critical("some_odd_query");
    finding.enrichment_path = "/tmp/thing";
    assert!(body(finding).contains("- **Review:** `/tmp/thing`"));
}
