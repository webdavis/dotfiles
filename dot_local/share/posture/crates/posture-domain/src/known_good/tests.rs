use super::*;

const HOME: &str = "/fixture";
const HASH: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
const PATH: &str = "/fixture/.local/libexec/osquery/space path";

fn tuple() -> KnownGoodTuple<'static> {
    KnownGoodTuple {
        digest: ManifestDigest::Built(HASH),
        mode: "0644",
        uid: "501",
        path: PATH,
    }
}

#[test]
fn strict_tuple_grammar_preserves_spaces_and_explicit_unbuilt() {
    let line = format!("{HASH} 0644 501 {PATH}");
    assert_eq!(KnownGoodTuple::parse_line(&line), Some(tuple()));
    assert_eq!(
        KnownGoodTuple::parse_line("unbuilt 0755 501 /posture"),
        Some(KnownGoodTuple {
            digest: ManifestDigest::Unbuilt,
            mode: "0755",
            uid: "501",
            path: "/posture"
        })
    );
}

#[test]
fn strict_grammar_bounds_hash_mode_uid_and_absolute_path_on_both_sides() {
    for (h, m, u, p) in [
        (&HASH[..63], "0644", "501", "/p"),
        (&format!("{HASH}a"), "0644", "501", "/p"),
        (&"g".repeat(64), "0644", "501", "/p"),
        (HASH, "644", "501", "/p"),
        (HASH, "00644", "501", "/p"),
        (HASH, "0684", "501", "/p"),
        (HASH, "0644", "", "/p"),
        (HASH, "0644", "12345678901", "/p"),
        (HASH, "0644", "-1", "/p"),
        (HASH, "0644", "501", "relative"),
        (HASH, "0644", "501", "/"),
        ("UNBUILT", "0755", "501", "/p"),
    ] {
        assert_eq!(
            KnownGoodTuple::parse_line(&format!("{h} {m} {u} {p}")),
            None,
            "{h} {m} {u} {p}"
        );
    }
    for uid in ["0", "0501", "1234567890"] {
        assert!(KnownGoodTuple::parse_line(&format!("{HASH} 0000 {uid} /p")).is_some());
        assert!(KnownGoodTuple::parse_line(&format!("{HASH} 7777 {uid} /p")).is_some());
    }
}

#[test]
fn audit_line_grammar_refuses_extra_separators_and_legacy_short_lines() {
    for line in [
        format!("{HASH}  0644 501 /p"),
        format!("{HASH}\t0644\t501\t/p"),
        format!("{HASH} /p"),
    ] {
        assert_eq!(KnownGoodTuple::parse_line(&line), None);
    }
}

#[test]
fn tuple_match_binds_each_column_to_the_exact_path() {
    assert!(tuple().matches(tuple()));
    for observed in [
        KnownGoodTuple {
            digest: ManifestDigest::Built(&"f".repeat(64)),
            ..tuple()
        },
        KnownGoodTuple {
            mode: "0640",
            ..tuple()
        },
        KnownGoodTuple {
            uid: "502",
            ..tuple()
        },
        KnownGoodTuple {
            path: "/other",
            ..tuple()
        },
    ] {
        assert!(!tuple().matches(observed));
    }
}

#[test]
fn hash_case_folds_but_mode_and_uid_remain_verbatim() {
    assert!(tuple().matches(KnownGoodTuple {
        digest: ManifestDigest::Built(&HASH.to_uppercase()),
        ..tuple()
    }));
    assert!(!tuple().matches(KnownGoodTuple {
        mode: "644",
        ..tuple()
    }));
    assert!(!tuple().matches(KnownGoodTuple {
        uid: "0501",
        ..tuple()
    }));
}

#[test]
fn unreadable_or_empty_columns_never_vouch_even_when_both_sides_are_empty() {
    for observed in [
        KnownGoodTuple {
            digest: ManifestDigest::Built(""),
            ..tuple()
        },
        KnownGoodTuple {
            mode: "",
            ..tuple()
        },
        KnownGoodTuple { uid: "", ..tuple() },
        KnownGoodTuple {
            path: "",
            ..tuple()
        },
    ] {
        assert!(!observed.matches(observed));
    }
}

#[test]
fn unbuilt_and_malformed_digest_never_vouch_for_forged_current_content() {
    for digest in [
        ManifestDigest::Unbuilt,
        ManifestDigest::Built("unbuilt"),
        ManifestDigest::Built("malformed"),
    ] {
        let observed = KnownGoodTuple { digest, ..tuple() };
        assert!(!observed.matches(observed));
        assert!(!observed.matches(tuple()));
        assert!(!tuple().matches(observed));
    }
}

mod tracking;
mod trust;
