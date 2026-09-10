//! The fixtures are real output, captured from this machine on 2026-09-09, not
//! output invented to match the parser.

use super::*;

const LIST: &str = "\
fd-find v8.4.0:
    fd
herdr-navigator v0.1.0 (https://github.com/devxplay/herdr.nvim.git?rev=deed8496356aab90e1cc364dac4f95d898fa6067#deed8496):
    herdr-navigator
nu v0.44.0:
    nu
    nu_plugin_core_match
selene v0.26.1:
    selene
";

#[test]
fn the_install_list_is_read_as_crates_with_their_versions_sources_and_binaries() {
    let crates = parse_install_list(LIST);
    assert_eq!(
        crates.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(),
        ["fd-find", "herdr-navigator", "nu", "selene"]
    );
    assert_eq!(crates[0].version, "8.4.0");
    assert_eq!(crates[0].source, Source::Registry);
    assert_eq!(crates[0].binaries, ["fd"]);
    assert_eq!(crates[2].binaries, ["nu", "nu_plugin_core_match"]);
}

#[test]
fn a_git_sourced_crate_is_skipped_and_its_rev_is_named() {
    // The registry cannot answer for a git install: crates.io has no version
    // of it to compare against, and its own remote is where a newer revision
    // would come from.
    let crates = parse_install_list(LIST);
    assert_eq!(
        crates[1].source,
        Source::Git {
            rev: "deed8496".to_string()
        }
    );
}

#[test]
fn a_header_with_an_origin_this_cannot_read_is_dropped_rather_than_called_a_registry_crate() {
    // Reading it as a registry crate would search crates.io for a version this
    // install never had, and then report the difference as an upgrade.
    for line in [
        "odd v1.0.0 (path+file:///somewhere):\n    odd\n",
        "odd v1.0.0 (https://example.git?rev=abc#):\n    odd\n",
    ] {
        assert!(parse_install_list(line).is_empty(), "{line}");
    }
}

#[test]
fn a_line_that_is_not_a_header_costs_that_line_and_no_other() {
    let text = format!("Installed packages:\nnot a header\n{LIST}");
    assert_eq!(parse_install_list(&text).len(), 4);
}

#[test]
fn an_indented_line_before_any_crate_is_dropped_rather_than_attached_to_nothing() {
    assert!(parse_install_list("    orphan\n").is_empty());
}

#[test]
fn the_search_line_for_the_crate_itself_is_the_newest_version_and_a_neighbour_is_not() {
    // Real `cargo search ripgrep` output: two of the three lines merely mention
    // ripgrep in their description, so position is not identity.
    let stdout = "\
ripgrep = \"15.2.0\"       # ripgrep is a line-oriented search tool.
gist-search = \"1.2.6\"    # Indexed code search for Rust - ripgrep-parity regex search.
cgx-core = \"0.1.0\"       # Core library for cgx.
";
    assert_eq!(parse_search(stdout, "ripgrep").as_deref(), Some("15.2.0"));
    assert_eq!(
        parse_search(stdout, "gist-search").as_deref(),
        Some("1.2.6")
    );
    assert_eq!(parse_search(stdout, "fd-find"), None);
}

#[test]
fn a_search_answer_of_another_shape_names_no_version_at_all() {
    for stdout in ["", "note: to learn more, run `cargo info`\n", "ripgrep\n"] {
        assert_eq!(parse_search(stdout, "ripgrep"), None, "{stdout:?}");
    }
}

#[test]
fn a_crate_behind_the_registry_produces_the_operators_sentence_naming_its_one_binary() {
    let crates = parse_install_list(LIST);
    assert_eq!(
        behind_sentence(&crates[0], "10.5.0"),
        "fd has a new version: 8.4.0 → 10.5.0. \
         Run the following command to compile it: cargo install fd-find"
    );
}

#[test]
fn a_crate_with_several_binaries_is_named_by_its_crate() {
    // Naming one of four binaries would read as one thing to do out of four.
    let crates = parse_install_list(LIST);
    assert_eq!(
        behind_sentence(&crates[2], "0.45.0"),
        "nu has a new version: 0.44.0 → 0.45.0. \
         Run the following command to compile it: cargo install nu"
    );
}

#[test]
fn a_crate_that_installed_no_binary_is_still_named_by_its_crate() {
    let installed = Installed {
        name: "libthing".to_string(),
        version: "1.0.0".to_string(),
        source: Source::Registry,
        binaries: Vec::new(),
    };
    assert!(behind_sentence(&installed, "2.0.0").starts_with("libthing has a new version: 1.0.0"));
}
