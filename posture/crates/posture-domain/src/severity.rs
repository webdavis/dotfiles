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

/// Which hermes route a submission of this tier belongs on.
///
/// `priority` is machine health and security ONLY (operator ruling
/// 2026-09-14), so a critical finding is the one thing posture puts there and
/// every lesser tier goes to posture's own channel, where a notice can be read
/// at leisure without teaching anyone to dismiss the loud channel.
///
/// `None` is not a default: it is a submission with no tier to read at all, the
/// heartbeat, the digest and the cursor-reset warning, and it leaves the
/// caller's configured route standing rather than inventing one.
pub fn severity_route(severity: Option<Severity>) -> Option<&'static str> {
    match severity? {
        Severity::Critical => Some("priority"),
        Severity::Notice | Severity::Info => Some("posture"),
    }
}

#[cfg(test)]
mod tests;
