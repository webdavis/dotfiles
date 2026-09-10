//! The fixture is real `rustup update` output, captured from this machine on
//! 2026-09-09, narration lines and all.

use super::*;

const UPDATE: &str = "\
info: syncing channel updates for nightly-aarch64-apple-darwin
info: latest update on 2026-09-10 for version 1.100.0-nightly (a36d05efa 2026-09-09)
info: removing previous version of component rust-src
info: downloading 7 components

  stable-aarch64-apple-darwin unchanged - rustc 1.98.1 (48a229cea 2026-09-01)
   nightly-aarch64-apple-darwin updated - rustc 1.100.0-nightly (a36d05efa 2026-09-09) (from rustc 1.92.0-nightly (0be8e1608 2025-09-19))

info: checking for self-update (current version: 1.29.1)
info: cleaning up downloads & tmp directories
";

#[test]
fn an_updated_toolchain_is_recorded_from_and_to() {
    let rows = parse_update_summary(UPDATE);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].name, "nightly-aarch64-apple-darwin");
    assert_eq!(
        rows[1].outcome,
        Outcome::Updated {
            from: "rustc 1.92.0-nightly (0be8e1608 2025-09-19)".to_string(),
            to: "rustc 1.100.0-nightly (a36d05efa 2026-09-09)".to_string(),
        },
        "both halves carry parentheses of their own, so the split is on ` (from `"
    );
}

#[test]
fn an_unchanged_toolchain_is_recorded_as_current() {
    let rows = parse_update_summary(UPDATE);
    assert_eq!(rows[0].name, "stable-aarch64-apple-darwin");
    assert_eq!(
        rows[0].outcome,
        Outcome::Unchanged("rustc 1.98.1 (48a229cea 2026-09-01)".to_string())
    );
}

#[test]
fn every_narration_line_around_the_summary_is_skipped() {
    // `info: latest update on 2026-09-10 for version ...` contains a dash-free
    // sentence about a version, and reading it as a row would invent one.
    assert_eq!(parse_update_summary(UPDATE).len(), 2);
}

#[test]
fn rustups_own_update_line_is_recorded() {
    // rustup reports itself in the same shape, so the same rule reads it.
    let rows = parse_update_summary("  rustup updated - 1.30.0 (from 1.29.1)\n");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "rustup");
    assert_eq!(
        rows[0].outcome,
        Outcome::Updated {
            from: "1.29.1".to_string(),
            to: "1.30.0".to_string(),
        }
    );
}

#[test]
fn a_summary_line_of_another_shape_is_skipped_rather_than_half_read() {
    for line in [
        // A verb this does not know: reading it as either known verb would
        // state the opposite of whatever it means half the time.
        "  stable-aarch64-apple-darwin removed - rustc 1.98.1\n",
        // Updated with no origin, so there is no `from` to record.
        "  stable updated - rustc 1.98.1\n",
        // An empty half on either side.
        "  stable unchanged - \n",
        "   unchanged - rustc 1.98.1\n",
        // Not a row at all.
        "info: cleaning up downloads & tmp directories\n",
        "\n",
    ] {
        assert!(parse_update_summary(line).is_empty(), "{line:?}");
    }
}

#[test]
fn a_run_that_printed_no_summary_at_all_names_no_toolchain() {
    assert!(parse_update_summary("").is_empty());
}
