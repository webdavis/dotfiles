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
/// `priority` is machine health and security ONLY (operator ruling 2026-09-14),
/// so a critical finding is the one thing posture would ever put there. It does
/// not go there yet, and this function is where that is held.
///
/// Two facts about the `priority` route, both measured against the live gateway:
/// it is signed with the retired Bash alerter's key rather than the one
/// `pns submit` signs with, so a page posted there answers 401; and its prompt
/// names `{alert.title}` and `{alert.detail}`, while a pns body carries `agent`,
/// `state`, `project`, `detail` and `request_id`. Hermes renders an unknown
/// placeholder as itself and a `deliver_only` route delivers the rendered
/// prompt, so reconciling the key alone would turn the 401 into a page reading
/// those two literals. A refused page is worse than a late one, because pns
/// reports a submission accepted off its own ledger rather than the
/// destination's answer, so posture advances its cursor and the finding is gone
/// with nothing in either channel.
///
/// So every tier goes to `posture`, where a page is read rather than dropped,
/// until `priority` carries the pns key AND a prompt naming a pns body's
/// fields. Flipping the critical arm below is the last line of the change that
/// settles both; the tests that pin the hold are what make the flip deliberate.
///
/// `None` is not a default: it is a submission with no tier to read at all, the
/// heartbeat, the digest and the cursor-reset warning, and it leaves the
/// caller's configured route standing rather than inventing one.
pub fn severity_route(severity: Option<Severity>) -> Option<&'static str> {
    match severity? {
        // HELD, not chosen. `priority` is where this belongs and cannot deliver
        // it in any configuration today; see above.
        Severity::Critical => Some("posture"),
        Severity::Notice | Severity::Info => Some("posture"),
    }
}

#[cfg(test)]
mod tests;
