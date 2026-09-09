use super::*;

#[test]
fn labels_require_two_ascii_characters_and_the_complete_allowed_alphabet() {
    for label in [
        "ab",
        "0a",
        "a.",
        "a@",
        "a_",
        "a-",
        "homebrew.mxcl.postgresql@17",
        "com.applex",
    ] {
        assert!(valid_allowlist_label(label), "{label}");
    }
    for label in [
        "",
        "a",
        ".a",
        "a/b",
        "a b",
        "a*",
        "éa",
        "com.apple",
        "COM.APPLE.foo",
        "a\n",
    ] {
        assert!(!valid_allowlist_label(label), "{label:?}");
    }
}

#[test]
fn plist_relativization_is_leading_only_while_program_replaces_all_home_occurrences() {
    assert_eq!(
        relativize_allowlist_identity(HOME, "/fixture/a//fixture/b", "/fixture/a//fixture/b"),
        ("~/a//fixture/b".into(), "~/a/~/b".into())
    );
    assert_eq!(
        relativize_allowlist_identity(HOME, "prefix/fixture/a", "prefix/fixture/a"),
        ("prefix/fixture/a".into(), "prefix~/a".into())
    );
    assert_eq!(
        relativize_allowlist_identity(HOME, "/fixture-neighbor/a", "/fixture-neighbor/a"),
        ("/fixture-neighbor/a".into(), "/fixture-neighbor/a".into())
    );
}

#[test]
fn allow_removes_all_matching_objects_and_appends_fresh_identity_after_preserved_lines() {
    let lines = [
        AllowlistLine::Preserved("# exact\tcomment"),
        AllowlistLine::Object {
            label: Some(ID.label),
            raw: "old-a",
        },
        AllowlistLine::Object {
            label: Some("org.other"),
            raw: "other",
        },
        AllowlistLine::Object {
            label: Some(ID.label),
            raw: "duplicate-a",
        },
        AllowlistLine::Object {
            label: None,
            raw: "{}",
        },
        AllowlistLine::Preserved(""),
    ];
    assert_eq!(
        curate_allowlist(&lines, AllowlistChange::Allow(entry())),
        Ok(vec![
            CuratedLine::Preserved("# exact\tcomment"),
            CuratedLine::Preserved("other"),
            CuratedLine::Preserved("{}"),
            CuratedLine::Preserved(""),
            CuratedLine::Added(entry())
        ])
    );
}

#[test]
fn deny_retains_every_other_line_in_order_without_appending() {
    let lines = [
        AllowlistLine::Object {
            label: Some(ID.label),
            raw: "drop",
        },
        AllowlistLine::Preserved("# retained"),
        AllowlistLine::Object {
            label: Some("org.other"),
            raw: "keep",
        },
    ];
    assert_eq!(
        curate_allowlist(&lines, AllowlistChange::Deny(ID.label)),
        Ok(vec![
            CuratedLine::Preserved("# retained"),
            CuratedLine::Preserved("keep")
        ])
    );
}

#[test]
fn malformed_or_multiple_value_line_refuses_the_whole_curation() {
    let lines = [
        AllowlistLine::Preserved("# prefix"),
        AllowlistLine::Invalid,
        AllowlistLine::Object {
            label: Some(ID.label),
            raw: "later",
        },
    ];
    for change in [
        AllowlistChange::Allow(entry()),
        AllowlistChange::Deny(ID.label),
    ] {
        assert_eq!(
            curate_allowlist(&lines, change),
            Err(CurationRefusal::InvalidLine(2))
        );
    }
}

#[test]
fn invalid_system_label_refuses_before_source_curation() {
    for label in ["a", "com.apple", "COM.APPLE.agent"] {
        assert_eq!(
            curate_allowlist::<&str>(&[], AllowlistChange::Deny(label)),
            Err(CurationRefusal::InvalidLabel)
        );
        assert_eq!(
            curate_allowlist::<&str>(
                &[],
                AllowlistChange::Allow(AllowlistEntry {
                    identity: LaunchdIdentity { label, ..ID },
                    ..entry()
                })
            ),
            Err(CurationRefusal::InvalidLabel)
        );
    }
}

#[test]
fn readding_first_entry_moves_it_last_but_readding_last_preserves_order() {
    let other = AllowlistLine::Object {
        label: Some("org.other"),
        raw: "other",
    };
    let own = AllowlistLine::Object {
        label: Some(ID.label),
        raw: "own",
    };
    let expected = Ok(vec![
        CuratedLine::Preserved("other"),
        CuratedLine::Added(entry()),
    ]);
    assert_eq!(
        curate_allowlist(&[own, other], AllowlistChange::Allow(entry())),
        expected
    );
    assert_eq!(
        curate_allowlist(&[other, own], AllowlistChange::Allow(entry())),
        expected
    );
}
