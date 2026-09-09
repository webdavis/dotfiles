use super::*;

fn manifests<'a>(pipeline: Manifest<'a>, managed_bin: Manifest<'a>) -> KnownGood<'a> {
    KnownGood {
        home: HOME,
        pipeline,
        managed_bin,
    }
}

#[test]
fn dedicated_pipeline_and_posture_trees_are_always_tracked() {
    let known = manifests(Manifest::Missing, Manifest::Trusted(&[]));
    for path in [
        PATH,
        "/fixture/.local/libexec/osquery/",
        "/fixture/.cargo/bin/posture",
    ] {
        assert!(known.is_tracked(path), "{path}");
    }
    for path in [
        "/fixture/.local/libexec/osquery",
        // The binary is matched exactly, so a cargo-installed neighbour whose
        // name merely starts with "posture" is not swept in beside it, and
        // neither is anything below a directory of that name.
        "/fixture/.cargo/bin/posturex",
        "/fixture/.cargo/bin/posture/a",
        "/else/.local/libexec/osquery/a",
    ] {
        assert!(!known.is_tracked(path), "{path}");
    }
}

#[test]
fn own_plists_are_home_anchored_and_allowlist_is_one_exact_file() {
    let known = manifests(Manifest::Missing, Manifest::Trusted(&[]));
    for path in [
        "/fixture/Library/LaunchAgents/com.webdavis.osquery-.plist",
        "/fixture/.config/osquery/page-launchd-allowlist.txt",
    ] {
        assert!(known.is_tracked(path));
    }
    for path in [
        "/Library/LaunchAgents/com.webdavis.osquery-a.plist",
        "/fixture/Library/LaunchAgents/other.plist",
        "/fixture/.config/osquery/page-launchd-allowlist.txt.lock",
        "/fixture/.config/osquery/webhook-secret",
    ] {
        assert!(!known.is_tracked(path), "{path}");
    }
}

#[test]
fn healthy_bin_manifest_tracks_only_its_named_paths_even_with_nested_bins() {
    let entries = [
        KnownGoodTuple {
            path: "/fixture/.local/bin/ours",
            ..tuple()
        },
        KnownGoodTuple {
            path: "/fixture/.local/bin/sub/ours",
            ..tuple()
        },
        KnownGoodTuple {
            path: "/fixture/.local/libexec/other/ours",
            ..tuple()
        },
    ];
    let known = manifests(Manifest::Missing, Manifest::Trusted(&entries));
    for entry in entries {
        assert!(known.is_tracked(entry.path));
    }
    for path in [
        "/fixture/.local/bin/thirdparty",
        "/fixture/.local/libexec/other/thirdparty",
        "/fixture/.local/bin",
        "/fixture/.local/bin-neighbor/ours",
    ] {
        assert!(!known.is_tracked(path), "{path}");
    }
}

#[test]
fn missing_unreadable_empty_or_untrustworthy_bin_manifest_tracks_every_bin_neighbor() {
    for manifest in [
        Manifest::Missing,
        Manifest::Unreadable,
        Manifest::Empty,
        Manifest::Untrustworthy,
    ] {
        let known = manifests(Manifest::Missing, manifest);
        for path in [
            "/fixture/.local/bin/thirdparty",
            "/fixture/.local/bin/sub/thirdparty",
            "/fixture/.local/libexec/other/tool",
        ] {
            assert!(known.is_tracked(path), "{manifest:?} {path}");
        }
        assert!(!known.is_tracked("/unrelated"));
    }
}

#[test]
fn nonempty_manifest_with_no_parsed_path_keeps_existing_membership_miss() {
    assert!(
        !manifests(Manifest::Missing, Manifest::Trusted(&[]))
            .is_tracked("/fixture/.local/bin/thirdparty")
    );
}

#[test]
fn exactly_one_manifest_is_selected_without_cross_vouch() {
    let bin = KnownGoodTuple {
        path: "/fixture/.local/bin/ours",
        ..tuple()
    };
    let posture = KnownGoodTuple {
        path: "/fixture/.cargo/bin/posture",
        ..tuple()
    };
    let pipeline = [tuple(), posture];
    let bins = [bin];
    let correct = manifests(Manifest::Trusted(&pipeline), Manifest::Trusted(&bins));
    let crossed = manifests(Manifest::Trusted(&bins), Manifest::Trusted(&pipeline));
    for observed in [tuple(), posture, bin] {
        assert!(correct.vouches(observed));
        assert!(!crossed.vouches(observed));
    }
    assert_eq!(manifest_for(HOME, posture.path), ManifestKind::Pipeline);
    assert_eq!(
        manifest_for(HOME, "/fixture/.local/libexec/posturex/a"),
        ManifestKind::ManagedBin
    );
    assert_eq!(manifest_for(HOME, "/other"), ManifestKind::Pipeline);
}

#[test]
fn every_unusable_manifest_refuses_suppression_in_both_arms() {
    for manifest in [
        Manifest::Missing,
        Manifest::Unreadable,
        Manifest::Empty,
        Manifest::Untrustworthy,
    ] {
        let known = manifests(manifest, manifest);
        assert!(!known.vouches(tuple()));
        assert!(!known.vouches(KnownGoodTuple {
            path: "/fixture/.local/bin/ours",
            ..tuple()
        }));
    }
}
