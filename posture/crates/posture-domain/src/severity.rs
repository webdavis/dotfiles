use crate::Detector;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Added,
    Removed,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtectionState {
    Off,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Critical,
    Notice,
    Info,
}

pub fn severity(detector: Detector, action: Action, protection: ProtectionState) -> Severity {
    use Detector::*;
    match detector {
        NewAdminUser | SuidBinUnexpected => Severity::Critical,
        FilevaultOff if action == Action::Added => Severity::Critical,
        FirewallState | GatekeeperState | SipState
            if action == Action::Added && protection == ProtectionState::Off =>
        {
            Severity::Critical
        }
        FilevaultOff
        | FilevaultState
        | FirewallState
        | GatekeeperState
        | RemoteAccessSharingState
        | SipState
        | PersistenceLaunchd
        | PersistenceLaunchdOverrides
        | PersistenceStartupItemsCrontab
        | KernelExtensionsNew
        | SystemExtensionsNew
        | FileEventsRecent
        | EsLaunchdWrites => Severity::Notice,
        _ => Severity::Info,
    }
}

#[cfg(test)]
mod tests;
