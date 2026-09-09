use super::*;

const KNOWN: &[(&str, Detector)] = &[
    ("new_admin_user", Detector::NewAdminUser),
    ("file_events_recent", Detector::FileEventsRecent),
    ("es_launchd_writes", Detector::EsLaunchdWrites),
    ("agent_authfile_changed", Detector::AgentAuthfileChanged),
    ("agent_binary_changed", Detector::AgentBinaryChanged),
    ("agent_exposure_changed", Detector::AgentExposureChanged),
    ("agent_secretfile_changed", Detector::AgentSecretfileChanged),
    ("chrome_extensions", Detector::ChromeExtensions),
    ("firefox_addons", Detector::FirefoxAddons),
    ("homebrew_packages", Detector::HomebrewPackages),
    ("installed_apps", Detector::InstalledApps),
    ("safari_extensions", Detector::SafariExtensions),
    ("kernel_extensions_new", Detector::KernelExtensionsNew),
    (
        "listening_ports_non_loopback",
        Detector::ListeningPortsNonLoopback,
    ),
    ("persistence_launchd", Detector::PersistenceLaunchd),
    (
        "persistence_launchd_overrides",
        Detector::PersistenceLaunchdOverrides,
    ),
    (
        "persistence_startup_items_crontab",
        Detector::PersistenceStartupItemsCrontab,
    ),
    ("recent_logins", Detector::RecentLogins),
    ("suid_bin_unexpected", Detector::SuidBinUnexpected),
    ("system_extensions_new", Detector::SystemExtensionsNew),
    ("filevault_off", Detector::FilevaultOff),
    ("filevault_state", Detector::FilevaultState),
    ("firewall_state", Detector::FirewallState),
    ("gatekeeper_state", Detector::GatekeeperState),
    (
        "remote_access_sharing_state",
        Detector::RemoteAccessSharingState,
    ),
    ("sip_state", Detector::SipState),
];

#[test]
fn all_26_detectors_are_admitted_bare_and_packed() {
    for &(query, detector) in KNOWN {
        assert_eq!(Detector::from_query(query), Some(detector), "{query}");
        assert_eq!(
            Detector::from_query(&format!("pack_owned-fixture_{query}")),
            Some(detector),
            "packed {query}"
        );
        assert_eq!(detector.query_name(), query);
    }
}

#[test]
fn unknown_names_and_the_heartbeat_canary_are_refused() {
    for query in [
        "heartbeat_canary",
        "unknown",
        "pack_owned_unknown",
        "pack__new_admin_user",
    ] {
        assert_eq!(Detector::from_query(query), None, "{query}");
    }
}

#[test]
fn only_one_nonempty_pack_segment_is_stripped() {
    assert_eq!(
        Detector::from_query("pack_agent-attack-surface_agent_exposure_changed"),
        Some(Detector::AgentExposureChanged)
    );
    assert_eq!(
        Detector::from_query("pack_with_underscore_new_admin_user"),
        None
    );
    assert_eq!(
        Detector::from_query("pack_owned_pack_owned_new_admin_user"),
        None
    );
}

#[test]
fn membership_baselines_are_discarded_but_nonbaseline_rows_survive() {
    assert!(!Detector::NewAdminUser.keeps(true, ""));
    assert!(Detector::NewAdminUser.keeps(false, ""));
}

#[test]
fn the_three_absolute_state_detectors_keep_their_baselines() {
    for detector in [
        Detector::FilevaultOff,
        Detector::RemoteAccessSharingState,
        Detector::AgentExposureChanged,
    ] {
        assert!(detector.keeps(true, ""), "{detector:?}");
    }
    assert!(!Detector::FilevaultState.keeps(true, ""));
}

#[test]
fn renameio_churn_is_discarded_on_every_admitted_detector() {
    for &(_, detector) in KNOWN {
        assert!(
            !detector.keeps(false, "/owned/.renameio-TempDir-a/file"),
            "{detector:?}"
        );
    }
    assert!(Detector::FileEventsRecent.keeps(false, "/owned/real-file"));
    assert!(Detector::FileEventsRecent.keeps(false, "/owned/not.renameio-TempDir/file"));
}

#[test]
fn the_enrich_path_names_the_file_each_detector_hands_the_enricher() {
    let paths = EnrichmentPaths {
        path: "/owned/file",
        target_path: "/owned/target",
        bundle_path: Some("/owned/bundle"),
    };
    for &(_, detector) in KNOWN {
        let expected = match detector {
            Detector::EsLaunchdWrites
            | Detector::PersistenceLaunchd
            | Detector::PersistenceStartupItemsCrontab
            | Detector::KernelExtensionsNew
            | Detector::SuidBinUnexpected => "/owned/file",
            Detector::FileEventsRecent => "/owned/target",
            Detector::SystemExtensionsNew => "/owned/bundle",
            _ => "",
        };
        assert_eq!(detector.enrichment_path(paths), expected, "{detector:?}");
    }
}

#[test]
fn only_an_absent_bundle_path_falls_back_to_the_file_path() {
    for (bundle_path, expected) in [(None, "/owned/file"), (Some(""), "")] {
        let paths = EnrichmentPaths {
            path: "/owned/file",
            bundle_path,
            ..EnrichmentPaths::default()
        };
        assert_eq!(
            Detector::SystemExtensionsNew.enrichment_path(paths),
            expected
        );
    }
}

#[test]
fn tabs_and_newlines_become_individual_spaces_in_the_enrich_path() {
    for (path, expected) in [
        ("/owned/a\tb\nc", "/owned/a b c"),
        ("/owned/a\t\nb", "/owned/a  b"),
        ("/owned/ordinary", "/owned/ordinary"),
    ] {
        let paths = EnrichmentPaths {
            path,
            ..EnrichmentPaths::default()
        };
        assert_eq!(Detector::EsLaunchdWrites.enrichment_path(paths), expected);
    }
}
