//! The home probe: is the operator's device on the home network?
//!
//! THE ROUTER IS THE WITNESS. The UDR (UniFi Dream Router) keeps the list of
//! clients currently on the wifi, and the device appearing in that list is
//! what "home" means here. The reading is a sensor only: nothing in the
//! delivery plan consumes it yet, because no row of the confirmed matrix
//! changes on home-ness until catch-up-on-return and the quiet window (part
//! 2's B and C) arrive to spend it. Building the integration ahead of the
//! consumer was considered and declined on 2026-08-25.
//!
//! THREE KEYS NAME THE DEVICE, and at least one is required. Any one of them
//! matching any client reads Home; the key the verdict NAMES is the strongest
//! that matched, scanning MAC, then hostname, then address. That order is the
//! disagreement rule: a MAC is the device itself, a client name is a label the
//! operator can move, and an address is only today's lease. A phone still
//! wants the NAME key, because iOS ships private wifi addresses and the MAC
//! the router sees is minted per network and can rotate (verified against the
//! live capture of 2026-08-20, where the phone's MAC is locally
//! administered); the MAC key is for devices whose MAC stays put. The name
//! match is exact and case-sensitive, because anything looser would let
//! "mister-2" answer for "mister".
//!
//! A KEY CAN GO STALE, and the DISAGREEMENT is how that shows: a rotated or
//! reassigned MAC names the wrong client while the verdict is still Home off
//! the hostname, and `device_ipv4` drifts under DHCP by design. One scan
//! records what every configured key found, the verdict is derived from it,
//! and a Home verdict with a key pointing at nobody or at somebody else is a
//! staleness, warned about once per state.
//!
//! FALSE STALENESS, the known ceiling: ONE physical device listed TWICE (wired
//! beside wireless, or a roaming re-association the router has not aged out)
//! puts the keys on different entries legitimately, and this reads that as a
//! disagreement. It takes two entries carrying DIFFERENT fields to get there:
//! a key the entry the verdict names ALSO carries is read off that entry, so
//! a duplicate answering to the same name changes nothing and cannot flip the
//! reading by being listed first. Merging entries is not attempted, because
//! the router gives no answer to "are these the same device" that is not a
//! guess; the evidence names the other client instead, so the operator can see
//! a duplicate for what it is.
//!
//! Fail direction: every failure to read is `Unknown`, never `NotHome`. The
//! future consumers suppress or replay on transitions, so inventing "the
//! device left" out of an unreachable router would fire a false transition;
//! Unknown is the reading that changes nothing.

// THE HOME-PROBE POLICY moved to `pns-domain`, one file per question it
// answers. What stays here presents the reading and setup guidance.
pub use pns_domain::home::{
    Client, DeviceIdentity, DeviceKey, HomePresence, HomeReading, KeyOutcome, KeyReading,
    Staleness, UNIFI_TYPE, episode_id, home_reading, is_new_staleness, stale_identifiers,
    stale_warning,
};

pub use pns_adapters::RouterSettings;
pub use pns_adapters::{UniFiRouter, first_site_id, parse_clients, read_home};

/// The seam one probe reads the router through. DECLARED in
/// `pns-application`, beside the home-probe use case that consumes it;
/// named here for the adapter that implements it.
pub use pns_application::Router;

/// The one line for the verdict itself. PURE for the same reason as its
/// caller: a swap of the two sentences below survived every suite before
/// this was a function of its own.
pub(super) fn verdict_line(presence: &HomePresence) -> String {
    match presence {
        // The matched value is DEBUG-QUOTED, the same escape `spell` gives a
        // config value: the value came from the router's own listing, so a
        // client name carrying a quote or a control byte would otherwise reach
        // a terminal verbatim. A plain name reads exactly as it did before.
        HomePresence::Home { matched_by, value } => format!(
            "home: on the home network (matched by {} {value:?})",
            matched_by.config_key()
        ),
        HomePresence::NotHome => {
            "home: NOT on the home network (no configured identifier matched a client)".to_string()
        }
        HomePresence::Unknown => {
            "home: unknown (router unreachable or its answer unreadable)".to_string()
        }
    }
}

mod setup;
pub use setup::*;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
#[path = "home/tests/settings.rs"]
mod settings_tests;

#[cfg(test)]
mod router_tests;

#[cfg(test)]
mod staleness_tests;
