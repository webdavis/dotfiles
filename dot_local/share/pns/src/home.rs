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
// answers. What stays here reads the router, the config and the terminal.
pub use pns_domain::home::identity::{
    Client, DeviceIdentity, DeviceKey, UNIFI_TYPE, normalized_mac,
};
pub use pns_domain::home::reading::{
    HomePresence, HomeReading, KeyOutcome, KeyReading, home_reading,
};
pub use pns_domain::home::staleness::{
    Staleness, episode_id, is_new_staleness, stale_identifiers, stale_warning,
};

/// The router sensor's settings, validated.
#[derive(Debug, PartialEq)]
pub struct RouterSettings {
    /// Where the router answers, e.g. `https://192.168.1.1`.
    pub router_url: String,
    /// The device to look for in the router's client list.
    pub device: DeviceIdentity,
}

/// The seam one probe reads the router through. The production impl carries
/// the deadline and the self-signed-TLS stance; a fake answers from a string.
pub trait Router {
    /// The clients listing as the router returned it, or `None` when it could
    /// not be fetched.
    fn clients_json(&self) -> Option<String>;
}

/// The clients in a UniFi `/clients` listing, or `None` when the text is not
/// one. `None` and an empty list are DIFFERENT readings: an empty list is a
/// parsed answer ("nobody is on the wifi"), while `None` is no answer.
pub fn parse_clients(clients_json: &str) -> Option<Vec<Client>> {
    let listing = serde_json::from_str::<serde_json::Value>(clients_json).ok()?;
    let clients = listing.get("data")?.as_array()?;
    // An INCOMPLETE PAGE is no answer: totalCount counts every client the
    // router knows, and a device beyond this page would read as departed,
    // which is a false transition. Completeness is judged on the ENTRIES,
    // which is what it has always been judged on.
    if let Some(total) = listing
        .get("totalCount")
        .and_then(serde_json::Value::as_u64)
        && total > clients.len() as u64
    {
        return None;
    }
    Some(
        clients
            .iter()
            .map(|client| {
                let field =
                    |key: &str| -> Option<String> { client.get(key)?.as_str().map(str::to_string) };
                Client {
                    name: field("name"),
                    ipv4: field("ipAddress"),
                    mac: field("macAddress"),
                }
            })
            .collect(),
    )
}

/// One reading: fetch through the seam, parse, judge.
pub fn read_home<R: Router>(router: &R, device: &DeviceIdentity) -> HomeReading {
    home_reading(
        router.clients_json().as_deref().and_then(parse_clients),
        device,
    )
}

/// The production router client, over the same HTTP stack as the other
/// native legs.
///
/// THE SECRET'S PATH IS THE POINT, exactly as in the moshi channel: the key
/// travels from the config into the request HEADER and nowhere else,
/// never argv, never a child's environment, never an error string. TLS
/// verification is disabled the way the hue bridge's is, and for the same
/// reason: the router serves a self-signed certificate for its own LAN
/// address, and no CA vouches for it.
pub struct UniFiRouter {
    /// The agent every call rides, INJECTED so a test can hand in one wearing
    /// a scripted transport (`Agent::with_parts`): the production pipeline
    /// runs for real and only the wire is fake, which is the closest Rust
    /// analog to stubbing Swift's URL Loading System.
    agent: ureq::Agent,
    /// e.g. `https://192.168.1.1`, from the `[plugins.router]` table.
    base: String,
    /// The API key, from the `[plugins.router]` table.
    key: String,
}

/// One bounded fetch: a probe on a diagnostic path is worth seconds, never a
/// hang.
const ROUTER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);

/// The router's answers are small (a 200-client listing measures kilobytes),
/// so the read is capped far below ureq's own 10MB default: a faulty router
/// streaming garbage costs at most this much memory before reading Unknown.
const ROUTER_BODY_CAP: u64 = 1_000_000;

impl UniFiRouter {
    /// The production wiring: TLS with verification disabled exactly as the
    /// hue bridge's is (the router serves a self-signed certificate for its
    /// own LAN address, and no CA vouches for it), no redirects, and the
    /// deadline on every call.
    pub fn new(base: String, key: String) -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(ROUTER_DEADLINE))
            .max_redirects(0)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .disable_verification(true)
                    .build(),
            )
            .build()
            .new_agent();
        Self::with_agent(agent, base, key)
    }

    /// The same router over any agent: the seam the scripted-transport tests
    /// inject through.
    pub fn with_agent(agent: ureq::Agent, base: String, key: String) -> Self {
        Self { agent, base, key }
    }
}

/// The integration API's clients listing for the default site.
///
/// The site is resolved by name (`default`) through the sites listing first,
/// because site ids are per-install; both calls ride one agent and one
/// deadline each.
impl Router for UniFiRouter {
    fn clients_json(&self) -> Option<String> {
        let get = |path: &str| {
            self.agent
                .get(format!("{}{path}", self.base))
                .header("X-API-KEY", &self.key)
                .call()
                .ok()?
                .body_mut()
                .with_config()
                .limit(ROUTER_BODY_CAP)
                .read_to_string()
                .ok()
        };
        let sites = get("/proxy/network/integration/v1/sites")?;
        let site = first_site_id(&sites)?;
        get(&format!(
            "/proxy/network/integration/v1/sites/{site}/clients?limit=200"
        ))
    }
}

/// The first site's id out of the sites listing, which on a UDR is the one
/// `default` site. Validated as id-shaped before it is joined into a path:
/// this is the one place a router answer becomes part of a URL.
pub fn first_site_id(sites_json: &str) -> Option<String> {
    let id = serde_json::from_str::<serde_json::Value>(sites_json)
        .ok()?
        .get("data")?
        .as_array()?
        .first()?
        .get("id")?
        .as_str()?
        .to_string();
    (!id.is_empty()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-'))
    .then_some(id)
}

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
mod reading_tests;

#[cfg(test)]
mod settings_tests;

#[cfg(test)]
mod router_tests;

#[cfg(test)]
mod staleness_tests;
