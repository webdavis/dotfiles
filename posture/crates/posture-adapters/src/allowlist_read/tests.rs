//! Read off `results-alerter/allowlist-verdict.sh`, whose file format this
//! shares with the writer in `allowlist_file.rs`.

use super::*;

const HOME: &str = "/Users/someone";

fn line(label: &str, path: &str, program: &str, sha: &str) -> String {
    serde_json::json!({"label": label, "path": path, "program": program, "sha256": sha}).to_string()
}

#[test]
fn a_tuple_arrives_with_its_three_fields_and_its_pin() {
    let text = line(
        "com.example.agent",
        "/tmp/a.plist",
        "/usr/bin/true",
        "abc123",
    );
    let allowlist = AllowlistText::parse(&text, HOME);
    let entries = allowlist.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].identity.label, "com.example.agent");
    assert_eq!(entries[0].identity.path, "/tmp/a.plist");
    assert_eq!(entries[0].identity.program, "/usr/bin/true");
    assert_eq!(entries[0].sha256, "abc123");
}

#[test]
fn a_home_relative_path_is_expanded_before_the_domain_compares_it() {
    // The committed seed file has to be user-agnostic, and the finding's path
    // is absolute, so the two only meet after this.
    let text = line(
        "com.example.agent",
        "~/Library/LaunchAgents/a.plist",
        "~/.local/bin/x",
        "",
    );
    let allowlist = AllowlistText::parse(&text, HOME);
    let entries = allowlist.entries();
    assert_eq!(
        entries[0].identity.path,
        "/Users/someone/Library/LaunchAgents/a.plist"
    );
    assert_eq!(entries[0].identity.program, "/Users/someone/.local/bin/x");
}

#[test]
fn only_a_leading_tilde_is_expanded_and_never_one_inside_a_path() {
    // A DELIBERATE NARROWING from the shell's global replace, which would turn
    // a real path containing `~/` into a different path. In a file whose
    // entries suppress a page, that confusion points the wrong way.
    let text = line("com.example.agent", "/tmp/~/odd/a.plist", "/bin/x", "");
    let allowlist = AllowlistText::parse(&text, HOME);
    assert_eq!(allowlist.entries()[0].identity.path, "/tmp/~/odd/a.plist");
}

#[test]
fn a_tilde_with_no_home_to_expand_against_is_left_exactly_as_stored() {
    // Expanding against an empty HOME would produce `/Library/...`, a path that
    // means something else entirely and is owned by root.
    let text = line(
        "com.example.agent",
        "~/Library/LaunchAgents/a.plist",
        "",
        "",
    );
    let allowlist = AllowlistText::parse(&text, "");
    assert_eq!(
        allowlist.entries()[0].identity.path,
        "~/Library/LaunchAgents/a.plist"
    );
}

#[test]
fn a_tuple_with_no_label_vouches_for_nothing_and_is_dropped() {
    // Keeping it would put an entry in the list whose empty label matches a
    // finding whose label osquery failed to read. Two absences are not an
    // agreement, and the agreement would SUPPRESS a persistence page.
    for text in [
        line("", "/tmp/a.plist", "/bin/x", ""),
        serde_json::json!({"path": "/tmp/a.plist"}).to_string(),
    ] {
        assert!(
            AllowlistText::parse(&text, HOME).entries().is_empty(),
            "{text}"
        );
    }
}

#[test]
fn a_line_that_is_not_a_tuple_costs_that_line_and_no_other() {
    let text = format!(
        "{}\nnot json\n\n[]\n\"a string\"\n{}\n",
        line("com.a", "/tmp/a", "/bin/a", ""),
        line("com.b", "/tmp/b", "/bin/b", ""),
    );
    let allowlist = AllowlistText::parse(&text, HOME);
    assert_eq!(
        allowlist
            .entries()
            .iter()
            .map(|e| e.identity.label)
            .collect::<Vec<_>>(),
        ["com.a", "com.b"]
    );
}

#[test]
fn a_tuple_with_no_pin_carries_an_empty_hash_rather_than_a_missing_entry() {
    // An entry with no pin is still an entry; the domain decides what an empty
    // pin means, and it is the arm that consults the manifest instead.
    let text =
        serde_json::json!({"label": "com.a", "path": "/tmp/a", "program": "/bin/a"}).to_string();
    let allowlist = AllowlistText::parse(&text, HOME);
    assert_eq!(allowlist.entries()[0].sha256, "");
}

#[test]
fn a_file_that_cannot_be_read_is_none_rather_than_an_empty_list() {
    // UNREADABLE IS NOT EMPTY. An empty list suppresses nothing, which is
    // correct, but it also says the list was consulted, which is a lie. The
    // caller turns `None` into the arm the domain pages on.
    let missing = std::env::temp_dir().join("posture-allowlist-absent-forever");
    assert!(AllowlistText::read(&missing, HOME).is_none());
}

#[test]
fn a_file_that_reads_but_holds_nothing_usable_is_an_empty_list() {
    // It WAS consulted, and it vouched for nothing.
    let path = std::env::temp_dir().join(format!("posture-allowlist-{}", std::process::id()));
    std::fs::write(&path, b"\n\nnot json\n").unwrap();
    let allowlist = AllowlistText::read(&path, HOME).expect("the file reads");
    assert!(allowlist.entries().is_empty());
    let _ = std::fs::remove_file(&path);
}
