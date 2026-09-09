#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detector {
    NewAdminUser,
    FileEventsRecent,
    EsLaunchdWrites,
    AgentAuthfileChanged,
    AgentBinaryChanged,
    AgentExposureChanged,
    AgentSecretfileChanged,
    ChromeExtensions,
    FirefoxAddons,
    HomebrewPackages,
    InstalledApps,
    SafariExtensions,
    KernelExtensionsNew,
    ListeningPortsNonLoopback,
    PersistenceLaunchd,
    PersistenceLaunchdOverrides,
    PersistenceStartupItemsCrontab,
    RecentLogins,
    SuidBinUnexpected,
    SystemExtensionsNew,
    FilevaultOff,
    FilevaultState,
    FirewallState,
    GatekeeperState,
    RemoteAccessSharingState,
    SipState,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnrichmentPaths<'a> {
    pub path: &'a str,
    pub target_path: &'a str,
    pub bundle_path: Option<&'a str>,
}

impl Detector {
    pub fn from_query(query: &str) -> Option<Self> {
        let bare = query
            .strip_prefix("pack_")
            .and_then(|packed| packed.split_once('_'))
            .filter(|(pack, _)| !pack.is_empty())
            .map_or(query, |(_, bare)| bare);
        match bare {
            "new_admin_user" => Some(Self::NewAdminUser),
            "file_events_recent" => Some(Self::FileEventsRecent),
            "es_launchd_writes" => Some(Self::EsLaunchdWrites),
            "agent_authfile_changed" => Some(Self::AgentAuthfileChanged),
            "agent_binary_changed" => Some(Self::AgentBinaryChanged),
            "agent_exposure_changed" => Some(Self::AgentExposureChanged),
            "agent_secretfile_changed" => Some(Self::AgentSecretfileChanged),
            "chrome_extensions" => Some(Self::ChromeExtensions),
            "firefox_addons" => Some(Self::FirefoxAddons),
            "homebrew_packages" => Some(Self::HomebrewPackages),
            "installed_apps" => Some(Self::InstalledApps),
            "safari_extensions" => Some(Self::SafariExtensions),
            "kernel_extensions_new" => Some(Self::KernelExtensionsNew),
            "listening_ports_non_loopback" => Some(Self::ListeningPortsNonLoopback),
            "persistence_launchd" => Some(Self::PersistenceLaunchd),
            "persistence_launchd_overrides" => Some(Self::PersistenceLaunchdOverrides),
            "persistence_startup_items_crontab" => Some(Self::PersistenceStartupItemsCrontab),
            "recent_logins" => Some(Self::RecentLogins),
            "suid_bin_unexpected" => Some(Self::SuidBinUnexpected),
            "system_extensions_new" => Some(Self::SystemExtensionsNew),
            "filevault_off" => Some(Self::FilevaultOff),
            "filevault_state" => Some(Self::FilevaultState),
            "firewall_state" => Some(Self::FirewallState),
            "gatekeeper_state" => Some(Self::GatekeeperState),
            "remote_access_sharing_state" => Some(Self::RemoteAccessSharingState),
            "sip_state" => Some(Self::SipState),
            _ => None,
        }
    }

    pub fn query_name(self) -> &'static str {
        match self {
            Self::NewAdminUser => "new_admin_user",
            Self::FileEventsRecent => "file_events_recent",
            Self::EsLaunchdWrites => "es_launchd_writes",
            Self::AgentAuthfileChanged => "agent_authfile_changed",
            Self::AgentBinaryChanged => "agent_binary_changed",
            Self::AgentExposureChanged => "agent_exposure_changed",
            Self::AgentSecretfileChanged => "agent_secretfile_changed",
            Self::ChromeExtensions => "chrome_extensions",
            Self::FirefoxAddons => "firefox_addons",
            Self::HomebrewPackages => "homebrew_packages",
            Self::InstalledApps => "installed_apps",
            Self::SafariExtensions => "safari_extensions",
            Self::KernelExtensionsNew => "kernel_extensions_new",
            Self::ListeningPortsNonLoopback => "listening_ports_non_loopback",
            Self::PersistenceLaunchd => "persistence_launchd",
            Self::PersistenceLaunchdOverrides => "persistence_launchd_overrides",
            Self::PersistenceStartupItemsCrontab => "persistence_startup_items_crontab",
            Self::RecentLogins => "recent_logins",
            Self::SuidBinUnexpected => "suid_bin_unexpected",
            Self::SystemExtensionsNew => "system_extensions_new",
            Self::FilevaultOff => "filevault_off",
            Self::FilevaultState => "filevault_state",
            Self::FirewallState => "firewall_state",
            Self::GatekeeperState => "gatekeeper_state",
            Self::RemoteAccessSharingState => "remote_access_sharing_state",
            Self::SipState => "sip_state",
        }
    }

    pub fn keeps(self, counter_is_zero: bool, target_path: &str) -> bool {
        // The existing predicate covers any admitted row carrying this path.
        !target_path.contains("/.renameio-TempDir")
            && (!counter_is_zero
                || matches!(
                    self,
                    Self::FilevaultOff
                        | Self::RemoteAccessSharingState
                        | Self::AgentExposureChanged
                ))
    }

    pub fn enrichment_path(self, paths: EnrichmentPaths<'_>) -> String {
        let path = match self {
            Self::EsLaunchdWrites
            | Self::PersistenceLaunchd
            | Self::PersistenceStartupItemsCrontab
            | Self::KernelExtensionsNew
            | Self::SuidBinUnexpected => paths.path,
            Self::FileEventsRecent => paths.target_path,
            Self::SystemExtensionsNew => paths.bundle_path.unwrap_or(paths.path),
            _ => "",
        };
        path.replace(['\t', '\n'], " ")
    }
}

#[cfg(test)]
mod tests;
