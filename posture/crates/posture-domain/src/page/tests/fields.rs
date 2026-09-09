//! What each detector shows, and what it refuses to show.

use super::{body, critical};
use crate::Triage;

#[test]
fn a_secret_or_credential_file_is_rendered_by_basename_never_with_its_path_or_its_content_hash() {
    for query in ["agent_secretfile_changed", "agent_authfile_changed"] {
        let mut finding = critical(query);
        finding.columns.path = Some("/home/me/.secret");
        let rendered = body(finding);
        assert!(
            rendered.contains("- **File:** `.secret`"),
            "{query}: {rendered}"
        );
        assert!(!rendered.contains("/home/me"), "{query} disclosed the path");
        assert!(!rendered.contains("sha"), "{query} disclosed a hash");
    }
}

#[test]
fn file_integrity_triage_facts_render_exactly_when_the_router_attached_them() {
    let mut without = critical("file_events_recent");
    without.columns.target_path = Some("/a/b");
    without.columns.category = Some("ssh");
    without.columns.action = Some("X");
    let rendered = body(without);
    assert!(!rendered.contains("Recorded"));
    assert!(!rendered.contains("Upgrade record"));

    let mut with = without;
    with.triage = Some(Triage {
        recorded: "aa",
        ondisk: "bb",
        upgrade: "brew 1.2",
    });
    let rendered = body(with);
    assert!(
        rendered.contains("- **Recorded:** `aa` · **On disk:** `bb`"),
        "{rendered}"
    );
    assert!(
        rendered.contains("- **Upgrade record:** `brew 1.2`"),
        "{rendered}"
    );
}

#[test]
fn an_embedded_newline_in_any_rendered_column_stays_on_one_line_so_no_signing_line_can_be_forged() {
    let mut finding = critical("persistence_launchd");
    finding.columns.label = Some("a\n- **Signing:** signed: Apple");
    let rendered = body(finding);
    assert!(
        rendered.contains("- **What:** `a - **Signing:** signed: Apple`"),
        "{rendered}"
    );
    // The forged text survives INSIDE the field, which is correct; what must
    // never happen is that it becomes a line of its own, because a line is what
    // the operator reads as ours.
    assert!(
        !rendered
            .lines()
            .any(|line| line.starts_with("- **Signing:**")),
        "a forged signing line escaped its field:\n{rendered}"
    );
}

#[test]
fn an_embedded_carriage_return_is_squashed_the_same_way_a_newline_is() {
    let mut finding = critical("persistence_launchd");
    finding.columns.label = Some("a\rb\tc");
    assert!(body(finding).contains("- **What:** `a b c`"));
}

#[test]
fn a_backtick_cannot_close_the_code_span_it_is_rendered_inside() {
    let mut finding = critical("persistence_launchd");
    finding.columns.label = Some("a`b**bold**");
    assert!(body(finding).contains("- **What:** `ab**bold**`"));
}

#[test]
fn a_signing_verdict_loses_its_backticks_and_asterisks_and_its_line_breaks() {
    // THE UNPINNED SIGNING EXCEPTION, captured from the renderer this replaces:
    // it strips MORE than a code-wrapped field, because it renders outside a
    // code span where an asterisk would be emphasis of its own.
    let mut finding = critical("persistence_launchd");
    finding.signing = Some(crate::Signing {
        untrusted: false,
        text: "we`ird *bold*\nsecond",
    });
    assert!(body(finding).contains("- **Signing:** weird bold second"));
}

#[test]
fn a_signing_verdict_takes_no_field_cap() {
    let long = "s".repeat(300);
    let mut finding = critical("persistence_launchd");
    finding.signing = Some(crate::Signing {
        untrusted: false,
        text: &long,
    });
    let rendered = body(finding);
    assert!(
        rendered.contains(&format!("- **Signing:** {long}")),
        "verdict was cut"
    );
}

#[test]
fn a_file_event_action_is_squashed_but_neither_code_wrapped_nor_capped() {
    // THE UNPINNED ACTION EXCEPTION, also captured from the renderer.
    let long = "z".repeat(300);
    let mut finding = critical("file_events_recent");
    finding.columns.target_path = Some("/etc/sudoers");
    finding.columns.category = Some("sudoers");
    finding.columns.action = Some("UP\nDA\tTED");
    assert!(body(finding).contains("- **Action:** UP DA TED"));

    let mut finding = critical("file_events_recent");
    finding.columns.target_path = Some("/etc/sudoers");
    finding.columns.action = Some(&long);
    assert!(
        body(finding).contains(&format!("- **Action:** {long}")),
        "action was cut"
    );
}

#[test]
fn a_file_event_falls_back_to_the_row_level_action() {
    let mut finding = critical("file_events_recent");
    finding.columns.target_path = Some("/a/b");
    finding.act = Some("CREATED");
    assert!(body(finding).contains("- **Action:** CREATED"));
}

#[test]
fn a_protection_shows_only_that_it_is_off() {
    assert_eq!(
        body(critical("gatekeeper_state")),
        "**Gatekeeper turned OFF**\n\
         - **State:** **OFF**\n\
         - Did you turn this off? If not, something else did - **investigate now**.\n\
         - Re-enable it in System Settings."
    );
}

#[test]
fn a_setuid_binary_shows_its_owner_after_the_signing_verdict() {
    let mut finding = critical("suid_bin_unexpected");
    finding.columns.path = Some("/tmp/x");
    finding.columns.username = Some("root");
    finding.signing = Some(crate::Signing {
        untrusted: true,
        text: "unsigned",
    });
    let rendered = body(finding);
    let signing = rendered.find("Signing").expect("a signing line");
    let owner = rendered.find("Owner").expect("an owner line");
    assert!(
        signing < owner,
        "the owner preceded the verdict:\n{rendered}"
    );
}

#[test]
fn the_identifier_fallback_stops_at_the_first_column_the_row_carries() {
    let mut finding = critical("some_odd_query");
    finding.columns.identifier = Some("ident");
    finding.columns.name = Some("name");
    assert!(body(finding).contains("- **What:** `ident`"));

    let mut empty = critical("some_odd_query");
    // An EMPTY label is still a label. The fallback stops here rather than
    // walking on, which is what the jq `//` chain did.
    empty.columns.label = Some("");
    empty.columns.name = Some("name");
    assert!(body(empty).contains("- **What:** ``"));
}

#[test]
fn a_row_with_no_identifying_column_renders_a_question_mark() {
    assert!(body(critical("some_odd_query")).contains("- **What:** `?`"));
}

#[test]
fn a_written_startup_item_falls_back_to_the_destination_filename() {
    let mut finding = critical("es_launchd_writes");
    finding.columns.path = Some("/bin/writer");
    finding.columns.dest_filename = Some("evil.plist");
    assert!(body(finding).contains("- **Wrote:** `evil.plist`"));
}
