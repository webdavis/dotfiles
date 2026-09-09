use super::*;
use crate::{Manifest, ManifestDigest};

const HASH: &str = "abcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcdefabcd";

fn tuple() -> KnownGoodTuple<'static> {
    KnownGoodTuple {
        digest: ManifestDigest::Built(HASH),
        mode: "0644",
        uid: "501",
        path: "/fixture/.local/libexec/osquery/script",
    }
}

#[test]
fn untracked_neighbor_is_silent_even_for_deletion_without_reading_current_state() {
    assert_eq!(
        integrity_verdict(false, "DELETED", "", FileKind::Other, |_| panic!(
            "neighbor"
        )),
        IntegrityVerdict::LogOnly
    );
}

#[test]
fn tracked_deletion_pages_before_any_current_state_read() {
    assert_eq!(
        integrity_verdict(true, "DELETED", "", FileKind::Regular, |_| panic!(
            "deleted"
        )),
        IntegrityVerdict::Page
    );
}

#[test]
fn symlink_and_nonregular_target_page_without_delay_or_vouch() {
    for kind in [FileKind::Symlink, FileKind::Other] {
        assert_eq!(
            integrity_verdict(true, "UPDATED", "", kind, |_| panic!("irregular")),
            IntegrityVerdict::Page
        );
    }
}

#[test]
fn only_empty_event_hash_requests_the_rename_delay_before_current_vouch() {
    for (hash, expected) in [
        ("", Rehash::AfterRenameDelay),
        (HASH, Rehash::Immediate),
        ("unbuilt", Rehash::Immediate),
    ] {
        let mut calls = Vec::new();
        assert_eq!(
            integrity_verdict(true, "UPDATED", hash, FileKind::Regular, |rehash| {
                calls.push(rehash);
                true
            }),
            IntegrityVerdict::LogOnly
        );
        assert_eq!(calls, [expected]);
    }
}

#[test]
fn current_state_answer_controls_suppression_independently_of_the_event_digest() {
    for hash in ["", HASH, "different"] {
        assert_eq!(
            integrity_verdict(true, "UPDATED", hash, FileKind::Regular, |_| false),
            IntegrityVerdict::Page
        );
        assert_eq!(
            integrity_verdict(true, "UPDATED", hash, FileKind::Regular, |_| true),
            IntegrityVerdict::LogOnly
        );
    }
}

#[test]
fn shared_deployed_verdict_refuses_unreadable_symlink_and_nonregular_state() {
    let entries = [tuple()];
    let known = KnownGood {
        home: "/fixture",
        pipeline: Manifest::Trusted(&entries),
        managed_bin: Manifest::Missing,
    };
    assert!(deployed_state_known_good(
        known,
        DeployedState::Regular(tuple())
    ));
    for state in [
        DeployedState::Unreadable,
        DeployedState::Symlink,
        DeployedState::Other,
    ] {
        assert!(!deployed_state_known_good(known, state));
    }
}

#[test]
fn stale_good_event_cannot_bless_replaced_current_bytes() {
    let entries = [tuple()];
    let known = KnownGood {
        home: "/fixture",
        pipeline: Manifest::Trusted(&entries),
        managed_bin: Manifest::Missing,
    };
    let current = KnownGoodTuple {
        digest: ManifestDigest::Built(&"f".repeat(64)),
        ..tuple()
    };
    assert_eq!(
        integrity_verdict(
            known.is_tracked(current.path),
            "UPDATED",
            HASH,
            FileKind::Regular,
            |_| deployed_state_known_good(known, DeployedState::Regular(current))
        ),
        IntegrityVerdict::Page
    );
}

#[test]
fn with_nothing_able_to_vouch_tracked_edits_page_while_neighbor_stays_silent() {
    let known = KnownGood {
        home: "/fixture",
        pipeline: Manifest::Missing,
        managed_bin: Manifest::Missing,
    };
    for (path, expected) in [
        (tuple().path, IntegrityVerdict::Page),
        ("/fixture/neighbor", IntegrityVerdict::LogOnly),
    ] {
        assert_eq!(
            integrity_verdict(
                known.is_tracked(path),
                "UPDATED",
                HASH,
                FileKind::Regular,
                |_| deployed_state_known_good(
                    known,
                    DeployedState::Regular(KnownGoodTuple { path, ..tuple() })
                )
            ),
            expected
        );
    }
}
