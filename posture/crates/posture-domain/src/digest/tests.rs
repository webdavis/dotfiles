//! Every expectation here was captured by RUNNING the jq renderer this
//! replaces, not read off its source. Each test names the fixture it came from
//! where the shape is not obvious from the assertion.

use super::*;

fn render_digest(entries: &[DigestEntry<'_>]) -> String {
    super::render_digest(entries, DigestLimits::default())
}

fn entry<'a>(detector: &'a str, identity: &'a str, summary: &'a str) -> DigestEntry<'a> {
    DigestEntry {
        detector: Some(detector),
        identity: Some(identity),
        summary: Some(summary),
    }
}

#[test]
fn nothing_spooled_renders_nothing_at_all() {
    // The caller reads an empty body as "say nothing today", so an empty spool
    // must not produce a header with no findings under it.
    assert_eq!(render_digest(&[]), "");
}

#[test]
fn one_finding_renders_its_detector_its_count_and_its_two_fields() {
    assert_eq!(
        render_digest(&[entry(
            "zebra_detector",
            "/etc/hosts",
            "zebra_detector /etc/hosts"
        )]),
        "**zebra_detector** (1)\n- `/etc/hosts` - `zebra_detector /etc/hosts`\n"
    );
}

#[test]
fn findings_group_by_detector_in_name_order_whatever_order_they_arrived_in() {
    // Captured from a spool whose zebra line was appended FIRST.
    let rendered = render_digest(&[
        entry("zebra_detector", "/etc/hosts", "s1"),
        entry("alpha_detector", "com.evil.agent", "s2"),
        entry("alpha_detector", "com.other.agent", "s3"),
    ]);
    assert_eq!(
        rendered,
        "**alpha_detector** (2)\n\
         - `com.evil.agent` - `s2`\n\
         - `com.other.agent` - `s3`\n\
         \n\
         **zebra_detector** (1)\n\
         - `/etc/hosts` - `s1`\n"
    );
}

#[test]
fn a_group_keeps_the_order_its_findings_were_spooled_in() {
    // Chronological within a group, so the bullets that survive the cap are the
    // day's earliest rather than an arbitrary ten.
    let rendered = render_digest(&[
        entry("one", "second-alphabetically-but-first-in-time", "s"),
        entry("one", "aaa", "s"),
    ]);
    let bullets: Vec<&str> = rendered.lines().filter(|l| l.starts_with("- ")).collect();
    assert_eq!(
        bullets,
        [
            "- `second-alphabetically-but-first-in-time` - `s`",
            "- `aaa` - `s`"
        ]
    );
}

#[test]
fn a_group_header_counts_every_finding_even_the_ones_the_bullet_cap_left_out() {
    // THE COUNT IS THE POINT: capping the lines a noisy detector spends must
    // never cap what the operator is told happened.
    let identities: Vec<String> = (0..13).map(|n| format!("i{n:02}")).collect();
    let many: Vec<DigestEntry<'_>> = identities
        .iter()
        .map(|identity| entry("one", identity, "s"))
        .collect();
    let rendered = render_digest(&many);
    assert!(rendered.starts_with("**one** (13)\n"), "{rendered}");
    assert_eq!(rendered.matches("- `i").count(), BULLETS_PER_GROUP);
    assert!(rendered.contains("\n… +3 more\n"), "{rendered}");
}

#[test]
fn a_group_at_the_bullet_cap_exactly_rolls_nothing_up() {
    let identities: Vec<String> = (0..BULLETS_PER_GROUP).map(|n| format!("i{n:02}")).collect();
    let exactly: Vec<DigestEntry<'_>> = identities
        .iter()
        .map(|identity| entry("one", identity, "s"))
        .collect();
    let rendered = render_digest(&exactly);
    assert!(!rendered.contains("more"), "{rendered}");
    assert_eq!(rendered.matches("- `i").count(), BULLETS_PER_GROUP);
}

#[test]
fn groups_past_the_group_cap_collapse_to_one_marker_that_says_how_many() {
    let names: Vec<String> = (1..=14).map(|n| format!("d{n:02}")).collect();
    let many: Vec<DigestEntry<'_>> = names
        .iter()
        .map(|name| entry(name, "i", "s"))
        .collect::<Vec<_>>();
    let rendered = render_digest(&many);
    assert_eq!(rendered.matches("** (1)").count(), GROUP_LIMIT);
    assert!(rendered.contains("**d12** (1)"), "{rendered}");
    assert!(!rendered.contains("**d13**"), "{rendered}");
    assert!(
        rendered.ends_with("… and 2 more detector group(s) - see results.log"),
        "{rendered}"
    );
}

#[test]
fn a_record_with_no_detector_groups_apart_from_one_whose_detector_is_a_question_mark() {
    // Both render `**?**`, which looks like a mistake and is not: merging them
    // would let a crafted `"detector":"?"` hide inside the malformed group's
    // count. The no-detector group sorts ahead of every named one.
    let rendered = render_digest(&[
        DigestEntry {
            detector: None,
            identity: Some("no-detector"),
            summary: Some("s1"),
        },
        entry("?", "literal-question", "s2"),
        entry("zzz", "named", "s3"),
    ]);
    assert_eq!(
        rendered,
        "**?** (1)\n\
         - `no-detector` - `s1`\n\
         \n\
         **?** (1)\n\
         - `literal-question` - `s2`\n\
         \n\
         **zzz** (1)\n\
         - `named` - `s3`\n"
    );
}

#[test]
fn an_absent_identity_or_summary_renders_as_a_question_mark() {
    let rendered = render_digest(&[DigestEntry {
        detector: Some("one"),
        identity: None,
        summary: None,
    }]);
    assert_eq!(rendered, "**one** (1)\n- `?` - `?`\n");
}

#[test]
fn a_backtick_cannot_close_the_span_it_is_rendered_inside() {
    // The field is attacker-influenceable: a launchd label, a path, a
    // certificate subject. A backtick that survived would end the inline-code
    // span and let everything after it render as live markdown.
    let rendered = render_digest(&[entry("one", "back`tick", "s")]);
    assert!(rendered.contains("- `backtick` - `s`"), "{rendered}");
}

#[test]
fn a_newline_cannot_forge_a_line_of_its_own() {
    let rendered = render_digest(&[entry("one", "i", "line\nbreak\there")]);
    assert!(rendered.contains("`line break here`"), "{rendered}");
    // A header and one bullet. Three would mean the crafted newline bought the
    // attacker a line the operator reads as ours.
    assert_eq!(rendered.lines().count(), 2, "{rendered}");
}

#[test]
fn a_field_longer_than_its_cap_is_cut_inside_its_own_span() {
    let long = "z".repeat(250);
    let rendered = render_digest(&[entry("one", &long, "s")]);
    let expected = format!("- `{}…(truncated)` - `s`", "z".repeat(240));
    assert!(rendered.contains(&expected), "{rendered}");
}

#[test]
fn a_body_over_its_cap_is_cut_with_a_marker_and_nothing_else() {
    // Twelve bullets of two 200-character fields overruns 1800; the marker adds
    // its own fourteen characters on top, which is the measured 1814.
    let longs: Vec<String> = (0..12)
        .map(|n| format!("{}{n:02}", "x".repeat(198)))
        .collect();
    let many: Vec<DigestEntry<'_>> = longs.iter().map(|long| entry("one", long, long)).collect();
    let rendered = render_digest(&many);
    assert_eq!(rendered.chars().count(), 1814);
    assert!(rendered.ends_with("\n… (truncated)"), "{rendered}");
}

#[test]
fn the_body_is_cut_by_characters_so_a_multi_byte_value_never_splits() {
    // A byte cut here would render a replacement glyph where the operator
    // expects a path, so the cut lands at the same COUNT whatever the encoding
    // costs.
    let wides: Vec<String> = (0..12)
        .map(|n| format!("{}{n:02}", "é".repeat(198)))
        .collect();
    let many: Vec<DigestEntry<'_>> = wides.iter().map(|wide| entry("one", wide, wide)).collect();
    let rendered = render_digest(&many);
    assert_eq!(rendered.chars().count(), 1814);
    assert!(rendered.chars().count() < rendered.len(), "{rendered}");
}

#[test]
fn a_body_exactly_at_its_cap_carries_no_truncation_marker() {
    // The boundary itself, exercised where it lives: no arrangement of entries
    // lands on 1800 exactly, because the per-field cap is 240.
    let exact = "p".repeat(BODY_LIMIT);
    assert_eq!(capped(exact.clone(), BODY_LIMIT), exact);
    let over = "p".repeat(BODY_LIMIT + 1);
    assert_eq!(
        capped(over, BODY_LIMIT),
        format!("{exact}{BODY_TRUNCATION}")
    );
}

#[test]
fn a_hundred_repeats_of_one_path_collapse_to_one_line_naming_the_count() {
    // The 2026-09-14 digest spent 110 lines on one rewritten agent config. The
    // header still counts every arrival; the body spends one line on them.
    let many: Vec<DigestEntry<'_>> = (0..100)
        .map(|_| {
            entry(
                "agent_authfile_changed",
                "~/.codex/config.toml",
                "agent_authfile_changed ~/.codex/config.toml",
            )
        })
        .collect();
    assert_eq!(
        render_digest(&many),
        "**agent_authfile_changed** (100)\n\
         - `~/.codex/config.toml` - `agent_authfile_changed ~/.codex/config.toml` (×100)\n"
    );
}

#[test]
fn two_paths_under_one_detector_stay_two_lines() {
    let rendered = render_digest(&[
        entry("one", "/first", "s1"),
        entry("one", "/second", "s2"),
        entry("one", "/first", "s1"),
    ]);
    assert_eq!(
        rendered,
        "**one** (3)\n- `/first` - `s1` (×2)\n- `/second` - `s2`\n"
    );
}

#[test]
fn two_different_things_said_about_one_path_stay_two_lines() {
    // What makes two findings the same finding is identity AND summary. A page
    // never reaches the spool at all (see the judge's own test), and within the
    // digest a differing summary is never folded away into another line's count.
    let rendered = render_digest(&[
        entry("one", "/etc/hosts", "one changed"),
        entry("one", "/etc/hosts", "one removed"),
    ]);
    assert_eq!(
        rendered,
        "**one** (2)\n- `/etc/hosts` - `one changed`\n- `/etc/hosts` - `one removed`\n"
    );
}

#[test]
fn a_finding_that_arrived_once_carries_no_count_suffix() {
    let rendered = render_digest(&[entry("one", "/etc/hosts", "s")]);
    assert_eq!(rendered, "**one** (1)\n- `/etc/hosts` - `s`\n");
    assert!(!rendered.contains('×'), "{rendered}");
}
