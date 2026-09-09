//! Which words the header uses, and which detector it decides that from.

use super::{body, critical};
use crate::{PageFinding, Severity};

/// The header line alone.
fn header_of(finding: PageFinding<'_>) -> String {
    body(finding).lines().next().unwrap_or_default().to_string()
}

#[test]
fn a_protection_reads_turned_off_at_crit_and_changed_below_it() {
    assert_eq!(
        header_of(critical("firewall_state")),
        "**Firewall turned OFF**"
    );
    let notice = PageFinding {
        severity: Severity::Notice,
        ..critical("firewall_state")
    };
    // Below critical it never reaches a page, so the wording is asserted at the
    // header rather than through a render that would drop the finding.
    assert_eq!(super::super::header::header(&notice), "Firewall changed");
}

#[test]
fn every_protection_query_has_its_own_plain_english_name() {
    for (query, expected) in [
        ("firewall_state", "**Firewall turned OFF**"),
        ("gatekeeper_state", "**Gatekeeper turned OFF**"),
        ("sip_state", "**System Integrity Protection turned OFF**"),
        ("filevault_state", "**FileVault turned OFF**"),
        ("filevault_off", "**FileVault turned OFF**"),
    ] {
        assert_eq!(header_of(critical(query)), expected, "{query}");
    }
}

#[test]
fn an_unmapped_query_renders_its_own_name_with_the_underscores_opened_out() {
    let mut finding = critical("some_odd_query");
    finding.columns.username = Some("bob");
    assert_eq!(body(finding), "**some odd query**\n- **What:** `bob`");
}

#[test]
fn a_watched_file_takes_its_header_from_its_category() {
    for (category, expected) in [
        ("ssh", "**SSH key file changed**"),
        ("authorized_keys", "**SSH key file changed**"),
        ("sudoers", "**sudoers changed**"),
        ("sshd_config", "**sshd_config changed**"),
        ("pipeline_integrity", "**Security tooling changed**"),
        ("allowlist_file", "**Allowlist changed**"),
        ("launch_agents", "**Startup folder changed**"),
        ("launch_daemons", "**Startup folder changed**"),
        ("something_else", "**Watched file changed**"),
    ] {
        let mut finding = critical("file_events_recent");
        finding.columns.category = Some(category);
        finding.columns.target_path = Some("/a/b");
        assert_eq!(header_of(finding), expected, "{category}");
    }
}

#[test]
fn our_own_agent_plist_reads_as_tooling_even_under_the_startup_category() {
    // THE ORDERING IS THE POINT. Our plists sit in the launch-agent category
    // beside every other startup file, so a category-first read would call a
    // change to the alerting pipeline an ordinary startup-folder change.
    let mut finding = critical("file_events_recent");
    finding.columns.category = Some("launch_agents");
    finding.columns.target_path =
        Some("/Users/x/Library/LaunchAgents/com.webdavis.osquery-digest.plist");
    assert_eq!(header_of(finding), "**Security tooling changed**");
}

#[test]
fn a_browser_extension_detector_reads_as_one() {
    for query in ["chrome_extensions", "firefox_addons", "safari_extensions"] {
        assert_eq!(
            header_of(critical(query)),
            "**New browser extension**",
            "{query}"
        );
    }
}

#[test]
fn each_remaining_detector_has_its_own_header() {
    for (query, expected) in [
        ("persistence_launchd", "**New startup item**"),
        (
            "persistence_launchd_overrides",
            "**Startup override changed**",
        ),
        (
            "persistence_startup_items_crontab",
            "**New startup/cron entry**",
        ),
        ("suid_bin_unexpected", "**New setuid root binary**"),
        ("new_admin_user", "**New administrator account**"),
        (
            "agent_exposure_changed",
            "**Agent port exposed off-loopback**",
        ),
        ("agent_authfile_changed", "**Agent credential changed**"),
        ("agent_secretfile_changed", "**Agent secret file changed**"),
        (
            "remote_access_sharing_state",
            "**Remote-access service enabled**",
        ),
        ("kernel_extensions_new", "**New kernel extension**"),
        ("system_extensions_new", "**New system extension**"),
        ("listening_ports_non_loopback", "**New network listener**"),
        ("recent_logins", "**Login**"),
        ("installed_apps", "**New app**"),
        ("homebrew_packages", "**New Homebrew package**"),
        ("es_launchd_writes", "**Startup item written by a process**"),
    ] {
        assert_eq!(header_of(critical(query)), expected, "{query}");
    }
}
