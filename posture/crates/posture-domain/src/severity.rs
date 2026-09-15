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
/// 2026-09-14), so a critical finding is the one thing posture ever puts
/// there. Everything below critical goes to `posture`, the channel read at
/// leisure.
///
/// TWO THINGS HAVE TO BE TRUE ON THE GATEWAY for a critical page to land, and
/// both are properties of the route rather than of this function: the route
/// must be signed with the key the sender holds for it, which is why delivery
/// carries ONE KEY PER ROUTE, and its prompt must name fields the posted body
/// actually carries. A page refused by the gateway is worse than a late one
/// when the sender reports acceptance off its own ledger rather than the
/// destination's answer, because the cursor then advances and the finding is
/// gone from both channels. The direct delivery path closes both halves: it
/// signs with the route's own key and posts a body serving both prompt shapes
/// the gateway's routes are written in.
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
