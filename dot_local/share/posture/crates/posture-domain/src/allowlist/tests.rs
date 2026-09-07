use super::*;

const HOME: &str = "/fixture";
const LIST: &str = "/fixture/.config/osquery/page-launchd-allowlist.txt";
const HASH: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";
const ID: LaunchdIdentity<'static> = LaunchdIdentity {
    label: "org.example.agent",
    path: "/fixture/Library/LaunchAgents/agent.plist",
    program: "/fixture/bin/agent",
};

fn entry() -> AllowlistEntry<'static> {
    AllowlistEntry {
        identity: LaunchdIdentity {
            path: "~/Library/LaunchAgents/agent.plist",
            program: "~/bin/agent",
            ..ID
        },
        sha256: "",
    }
}

fn judge(
    entries: Allowlist<'_>,
    identity: LaunchdIdentity<'_>,
    hash: Option<&str>,
    vouch: impl FnMut(&str) -> bool,
) -> AllowlistVerdict {
    allowlist_verdict(HOME, LIST, entries, identity, hash, vouch)
}

#[test]
fn full_own_agent_tuple_requires_plist_then_allowlist_vouches() {
    let mut calls = Vec::new();
    assert_eq!(
        judge(Allowlist::Read(&[entry()]), ID, None, |path| {
            calls.push(path.to_owned());
            true
        }),
        AllowlistVerdict::Suppress
    );
    assert_eq!(calls, [ID.path, LIST]);
}

#[test]
fn missing_or_unreadable_allowlist_cannot_suppress() {
    for list in [Allowlist::Unreadable, Allowlist::Read(&[])] {
        assert_eq!(
            judge(list, ID, Some(HASH), |_| panic!("no vouch on a miss")),
            AllowlistVerdict::NotAllowlisted
        );
    }
}

#[test]
fn either_empty_identity_column_degrades_without_vouch() {
    for identity in [
        LaunchdIdentity { path: "", ..ID },
        LaunchdIdentity { program: "", ..ID },
    ] {
        let entries = [AllowlistEntry {
            identity,
            sha256: HASH,
        }];
        assert_eq!(
            judge(Allowlist::Read(&entries), ID, Some(HASH), |_| panic!(
                "degraded entry"
            )),
            AllowlistVerdict::NotAllowlisted
        );
    }
}

#[test]
fn path_or_program_divergence_is_reused_before_any_vouch() {
    for identity in [
        LaunchdIdentity {
            path: "/other",
            ..ID
        },
        LaunchdIdentity {
            program: "/other",
            ..ID
        },
    ] {
        assert_eq!(
            judge(
                Allowlist::Read(&[entry()]),
                identity,
                Some(HASH),
                |_| panic!("reused identity")
            ),
            AllowlistVerdict::ReusedLabel
        );
    }
}

#[test]
fn first_matching_entry_wins_even_when_degraded_or_reused() {
    for (first, expected) in [
        (
            LaunchdIdentity { path: "", ..ID },
            AllowlistVerdict::NotAllowlisted,
        ),
        (
            LaunchdIdentity {
                program: "/other",
                ..ID
            },
            AllowlistVerdict::ReusedLabel,
        ),
    ] {
        let entries = [
            AllowlistEntry {
                identity: first,
                sha256: "",
            },
            entry(),
        ];
        assert_eq!(
            judge(Allowlist::Read(&entries), ID, None, |_| panic!(
                "first entry already decides"
            )),
            expected
        );
    }
}

#[test]
fn unknown_label_does_not_consume_vouch() {
    assert_eq!(
        judge(
            Allowlist::Read(&[entry()]),
            LaunchdIdentity {
                label: "org.other",
                ..ID
            },
            None,
            |_| panic!("label miss")
        ),
        AllowlistVerdict::NotAllowlisted
    );
}

#[test]
fn current_pin_match_skips_plist_vouch_but_requires_list_vouch() {
    let entries = [AllowlistEntry {
        sha256: HASH,
        ..entry()
    }];
    let mut calls = Vec::new();
    assert_eq!(
        judge(Allowlist::Read(&entries), ID, Some(HASH), |path| {
            calls.push(path.to_owned());
            true
        }),
        AllowlistVerdict::Suppress
    );
    assert_eq!(calls, [LIST]);
    assert_eq!(
        judge(Allowlist::Read(&entries), ID, Some(HASH), |_| false),
        AllowlistVerdict::NotAllowlisted
    );
}

#[test]
fn changed_missing_or_differently_cased_current_hash_is_reused() {
    let entries = [AllowlistEntry {
        sha256: HASH,
        ..entry()
    }];
    for hash in [
        None,
        Some(""),
        Some("changed"),
        Some(HASH.to_uppercase().as_str()),
    ] {
        assert_eq!(
            judge(Allowlist::Read(&entries), ID, hash, |_| panic!(
                "pin divergence"
            )),
            AllowlistVerdict::ReusedLabel
        );
    }
}

#[test]
fn unpinned_plist_refusal_does_not_consume_final_list_vouch() {
    let mut calls = Vec::new();
    assert_eq!(
        judge(Allowlist::Read(&[entry()]), ID, None, |path| {
            calls.push(path.to_owned());
            false
        }),
        AllowlistVerdict::NotAllowlisted
    );
    assert_eq!(calls, [ID.path]);
}

#[test]
fn unpinned_plist_match_cannot_bypass_final_list_refusal() {
    let mut calls = Vec::new();
    assert_eq!(
        judge(Allowlist::Read(&[entry()]), ID, None, |path| {
            calls.push(path.to_owned());
            path == ID.path
        }),
        AllowlistVerdict::NotAllowlisted
    );
    assert_eq!(calls, [ID.path, LIST]);
}

#[test]
fn expansion_preserves_bash_replacement_of_embedded_home_tokens() {
    let identity = LaunchdIdentity {
        path: "prefix~/one/~/two",
        program: "~/bin/~/agent",
        ..ID
    };
    let finding = LaunchdIdentity {
        path: "prefix/fixture/one//fixture/two",
        program: "/fixture/bin//fixture/agent",
        ..ID
    };
    assert_eq!(
        judge(
            Allowlist::Read(&[AllowlistEntry {
                identity,
                sha256: ""
            }]),
            finding,
            None,
            |_| true
        ),
        AllowlistVerdict::Suppress
    );
}

#[test]
fn hostile_field_bytes_do_not_shift_identity_columns() {
    for identity in [
        LaunchdIdentity {
            path: "/fixture/Library/LaunchAgents/agent.plist\u{1f}/other",
            ..ID
        },
        LaunchdIdentity {
            program: "/fixture/bin/agent\t/other",
            ..ID
        },
    ] {
        assert_eq!(
            judge(Allowlist::Read(&[entry()]), identity, None, |_| true),
            AllowlistVerdict::ReusedLabel
        );
    }
    assert_eq!(
        judge(
            Allowlist::Read(&[entry()]),
            LaunchdIdentity {
                label: "org.example.agent\nother",
                ..ID
            },
            None,
            |_| true
        ),
        AllowlistVerdict::NotAllowlisted
    );
}

mod curation;
mod routing;
