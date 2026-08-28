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

/// One way a device can be recognized in the router's client list. Strength
/// runs MAC, then hostname, then address: a MAC is the device itself, a client
/// name is what the operator called it, and an address is whatever DHCP handed
/// out today.
///
/// THE ORDER THE VARIANTS ARE DECLARED IN IS NOT THAT RULE. Nothing derives an
/// ordering off it and nothing iterates the variants, so reversing this enum
/// leaves the whole suite green; the statement order of `home_reading`'s three
/// `if let` blocks is the only thing that decides which key a Home verdict
/// names.
///
/// A FOURTH VARIANT COMPILES WITHOUT REACHING THREE PLACES, and none of them
/// fails when it is missed: `device_identity`'s key reads (the key is never
/// read out of the config), `home_reading`'s scan (it never matches a client),
/// and the `NoDeviceIdentifier` line's key list (the operator is never told the
/// key exists). The compiler asks only about the two exhaustive matches,
/// `config_key` here and the shape in `setup_report`, so those three are hand
/// edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKey {
    Mac,
    Hostname,
    Ipv4,
}

impl DeviceKey {
    /// The key's spelling in `[plugins.router]`, which is also how every line
    /// names it: the operator reads back the word they typed.
    pub fn config_key(self) -> &'static str {
        match self {
            DeviceKey::Mac => "device_mac",
            DeviceKey::Hostname => "device_hostname",
            DeviceKey::Ipv4 => "device_ipv4",
        }
    }
}

/// The configured device, as identifiers that have already been VALIDATED:
/// an address is an `Ipv4Addr` and never text, a MAC is the one normalized
/// spelling, and at least one of the three is set. `device_identity` is the
/// only way to build one, so an unparsed value cannot reach a comparison.
#[derive(Debug, PartialEq)]
pub struct DeviceIdentity {
    hostname: Option<String>,
    ipv4: Option<std::net::Ipv4Addr>,
    mac: Option<String>,
}

/// What the router said about the device. `NotHome` requires a PARSED client
/// list that no configured key matched; anything less is `Unknown`.
///
/// HOME CARRIES ITS OWN EVIDENCE. The key that answered and the value it
/// answered with live inside the `Home` variant rather than beside the
/// verdict, so there is no matched-key field for a `NotHome` to fill in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HomePresence {
    Home {
        matched_by: DeviceKey,
        value: String,
    },
    NotHome,
    Unknown,
}

/// What ONE configured key found in the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The client the verdict names.
    MatchedDevice,
    /// A DIFFERENT client than the one the verdict names, already SPELLED
    /// for printing the way `SetupFailure::InvalidDeviceKey`'s `found` is.
    MatchedOtherClient { client: String },
    /// No client in the listing carried this value.
    MatchedNothing,
}

/// One configured key, its value, and what that value found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyReading {
    pub key: DeviceKey,
    pub value: String,
    pub outcome: KeyOutcome,
}

/// One reading of the listing: the verdict, plus what EVERY configured key
/// found on the way to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeReading {
    pub presence: HomePresence,
    pub keys: Vec<KeyReading>,
}

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

/// One client the router listed, in the router's own spelling. Every field is
/// optional because every one of them is: the UDR omits `name` for a client
/// it has not identified, and a client can be listed before it has an
/// address. Nothing here is validated, because the router is not the operator:
/// an entry this probe cannot read is a client that matches nothing, never a
/// listing that failed.
#[derive(Debug, PartialEq)]
pub struct Client {
    name: Option<String>,
    ipv4: Option<String>,
    mac: Option<String>,
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

/// One reading for one configured device against one parsed listing: what
/// EVERY configured key found, and the verdict DERIVED from that.
///
/// ONE IMPLEMENTATION OF PRECEDENCE. The verdict is not a second scan beside
/// the evidence: it is the first key of the same scan that found anything, so
/// the line naming the winner and the lines describing the keys cannot drift
/// apart.
pub fn home_reading(clients: Option<Vec<Client>>, device: &DeviceIdentity) -> HomeReading {
    let Some(clients) = clients else {
        // NOTHING WAS SEARCHED, so there is nothing to say a key found: an
        // unreachable router is not a listing in which every key came up
        // empty.
        return HomeReading {
            presence: HomePresence::Unknown,
            keys: Vec::new(),
        };
    };
    // STRONGEST KEY FIRST, and the FIRST one that matched anything wins.
    // ANY match is Home, so the order is only ever read on disagreement,
    // which is exactly the operator's rule: when the keys agree the winner's
    // identity does not matter, and when they point at different clients the
    // strongest key is the one that says which client the device is. A key
    // that matches nothing is skipped and never a failure. This statement
    // order is the whole precedence rule.
    let configured: Vec<(DeviceKey, String)> = [
        device.mac.clone().map(|mac| (DeviceKey::Mac, mac)),
        device
            .hostname
            .clone()
            .map(|hostname| (DeviceKey::Hostname, hostname)),
        device.ipv4.map(|ipv4| (DeviceKey::Ipv4, ipv4.to_string())),
    ]
    .into_iter()
    .flatten()
    .collect();
    let first_match = |key: DeviceKey| {
        clients
            .iter()
            .position(|client| client_carries(client, key, device))
    };
    // A COMPLETE listing that no configured key matched is the only NotHome
    // there is: every unreadable, unparseable or incomplete answer left
    // through the Unknown above.
    let (presence, winner_at) = match configured
        .iter()
        .find_map(|(key, value)| first_match(*key).map(|at| (*key, value.clone(), at)))
    {
        Some((key, value, at)) => (
            HomePresence::Home {
                matched_by: key,
                value,
            },
            Some(at),
        ),
        None => (HomePresence::NotHome, None),
    };
    HomeReading {
        presence,
        keys: configured
            .into_iter()
            .map(|(key, value)| KeyReading {
                key,
                value,
                // MEMBERSHIP IN THE WINNER'S ENTRY is what "this device"
                // means, not an index comparison against wherever this key
                // matched first. A listing can carry one value twice (a
                // duplicate entry, a name two clients answer to), and asking
                // "which entry did this key find first" then answers out of
                // the router's listing ORDER: reverse two entries and the
                // same physical state flips between agreeing and stale, so
                // the episode flaps on nothing. Asking whether the entry the
                // verdict names carries this value has no order in it. Only
                // a key that entry does NOT carry needs a first match, and
                // that one is evidence rather than identity.
                outcome: if winner_at.is_some_and(|at| client_carries(&clients[at], key, device)) {
                    KeyOutcome::MatchedDevice
                } else {
                    match first_match(key) {
                        Some(at) => KeyOutcome::MatchedOtherClient {
                            client: client_label(&clients[at]),
                        },
                        None => KeyOutcome::MatchedNothing,
                    }
                },
            })
            .collect(),
    }
}

/// Whether ONE listed client carries the value of ONE configured key: the
/// single implementation of "this key matches that entry", asked to find the
/// verdict and asked again of the entry the verdict names.
fn client_carries(client: &Client, key: DeviceKey, device: &DeviceIdentity) -> bool {
    match key {
        // The router's spelling is normalized into the one the config was
        // validated into, so a listing writing `2E-11-AB-6D-B0-4F` still
        // matches `2e:11:ab:6d:b0:4f`.
        DeviceKey::Mac => device.mac.as_deref().is_some_and(|mac| {
            client.mac.as_deref().and_then(normalized_mac).as_deref() == Some(mac)
        }),
        // EXACT and case-sensitive: anything looser would let "mister-2"
        // answer for "mister".
        DeviceKey::Hostname => device
            .hostname
            .as_deref()
            .is_some_and(|hostname| client.name.as_deref() == Some(hostname)),
        // PARSED rather than compared as text, so a listing's own spelling of
        // an address cannot miss the one the config holds.
        DeviceKey::Ipv4 => device.ipv4.is_some_and(|ipv4| {
            client
                .ipv4
                .as_deref()
                .and_then(|text| text.parse::<std::net::Ipv4Addr>().ok())
                == Some(ipv4)
        }),
    }
}

/// A Home verdict with at least one configured key pointing somewhere else:
/// the key that answered, and every key that disagrees with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Staleness {
    pub winner: DeviceKey,
    pub disagreeing: Vec<KeyReading>,
}

/// The staleness in one reading, or `None` when the keys have nothing to
/// disagree about.
///
/// AWAY IS NOT STALE. A NotHome reading has every key matching nothing, which
/// is what being away IS, and an Unknown searched nothing at all: warning on
/// either would fire every time the operator left the house. ONE configured
/// key is never stale either, because a lone key has nothing to disagree
/// with, whatever it found.
pub fn stale_identifiers(reading: &HomeReading) -> Option<Staleness> {
    let HomePresence::Home { matched_by, .. } = &reading.presence else {
        return None;
    };
    let disagreeing: Vec<KeyReading> = reading
        .keys
        .iter()
        .filter(|key| key.outcome != KeyOutcome::MatchedDevice)
        .cloned()
        .collect();
    (!disagreeing.is_empty()).then_some(Staleness {
        winner: *matched_by,
        disagreeing,
    })
}

/// The canonical spelling of one stale STATE, and nothing else: the key that
/// answered, then every disagreeing key with what it found, in precedence
/// order.
///
/// IT EXCLUDES EVERY VALUE THAT CAN MOVE ON ITS OWN. No matched value, no
/// label for the other client, no client count and no time, because
/// `device_ipv4` drifting under DHCP is the same stale state it was
/// yesterday and the operator has already been told. A key moving between
/// `none` and `other`, joining or leaving the stale set, or a different key
/// answering, is a different state and is news again.
pub fn episode_id(staleness: &Staleness) -> String {
    std::iter::once(staleness.winner.config_key().to_string())
        .chain(staleness.disagreeing.iter().map(|key| {
            format!(
                "{}={}",
                key.key.config_key(),
                match key.outcome {
                    // Never reached from `stale_identifiers`, which keeps
                    // only the keys that disagree; spelled rather than
                    // panicked on, because an identity is not the place to
                    // discover an impossible state.
                    KeyOutcome::MatchedDevice => "device",
                    KeyOutcome::MatchedOtherClient { .. } => "other",
                    KeyOutcome::MatchedNothing => "none",
                }
            )
        }))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Whether a staleness is worth saying out loud, given what was said last.
///
/// A RESOLVED staleness is news to nobody: the operator was told about a
/// disagreement that is no longer there, and an all-clear for a warning they
/// may never have read is one more thing to read.
pub fn is_new_staleness(remembered: Option<&str>, current: Option<&str>) -> bool {
    current.is_some_and(|episode| remembered != Some(episode))
}

/// The client a disagreeing key found, as the operator can recognize it:
/// the first field that identifies it, its name, then its MAC, then its
/// address. SPELLED here rather than at the print, the way
/// `SetupFailure::InvalidDeviceKey`'s `found` is, so the router's own text
/// reaches a terminal as its escape and the nameless case reads as prose.
///
/// PRESENT BUT EMPTY IS NOT IDENTIFYING, the same filter `router_settings`
/// puts on a blank `brand`: the router lists a field it has no answer for as
/// `""` as readily as it omits it, and `matched a different client ""` names
/// nothing the operator can act on.
///
/// A client a key MATCHED always carries at least the field that matched it,
/// so the last arm is the floor of a total function rather than a case the
/// router produces.
fn client_label(client: &Client) -> String {
    let identifying = |field: &Option<String>| {
        field
            .as_deref()
            .filter(|text| !text.is_empty())
            .map(str::to_string)
    };
    match identifying(&client.name)
        .or_else(|| identifying(&client.mac))
        .or_else(|| identifying(&client.ipv4))
    {
        Some(text) => format!("{text:?}"),
        None => "an unnamed client".to_string(),
    }
}

/// The enabled router sensor's settings table, or the cause it could not be
/// had. The probe's config home is `[plugins.router]`, the sensor registered
/// in the roster, so the whole file is read by one schema and the operator has
/// one spelling to get right.
pub fn enabled_router_table(config: &crate::config::Config) -> Result<&toml::Table, SetupFailure> {
    let entry = config
        .plugins
        .get("router")
        .ok_or(SetupFailure::NoRouterPlugin)?;
    // A probe the operator SWITCHED OFF is not one they never wrote: one is
    // fixed by flipping the flag in front of them, the other by writing a
    // table, and a single "not configured" line sends half of them to the
    // wrong edit.
    if !entry.enabled {
        return Err(SetupFailure::RouterDisabled);
    }
    Ok(&entry.settings)
}

/// The one brand a compiled-in backend answers. It is VALIDATED and then
/// discarded: `trait Router` is the seam a second backend enters through, and
/// the enum that dispatches between two of them is worth writing the day
/// there are two.
const UNIFI_BRAND: &str = "unifi";

/// The settings out of the router sensor's table, or the cause they could not
/// be had. The BRAND is settled first, because every setting under it belongs
/// to whichever router it names.
pub fn router_settings(router: &toml::Table) -> Result<RouterSettings, SetupFailure> {
    // Present but EMPTY is the key left blank, the same hole as absent: the
    // filter is what keeps `brand = ""` from being quoted back as a brand no
    // backend answers, which names a value the operator never typed.
    let brand = router
        .get("brand")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(SetupFailure::NoBrand)?;
    if brand != UNIFI_BRAND {
        return Err(SetupFailure::UnknownBrand(brand.to_string()));
    }
    // Present but empty is a hole, not a value, and present but the wrong
    // type is refused rather than coerced: both are one line for the operator
    // to fix, and neither is a router this probe could reach.
    let router_url = router
        .get("router_url")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or(SetupFailure::InvalidRouterTable)?;
    Ok(RouterSettings {
        router_url,
        device: device_identity(router)?,
    })
}

/// The configured device out of the router's table, or the cause it could
/// not be had. ABSENT IS NOT CONFIGURED, and all three absent is refused: a
/// probe with nothing to look for would read NotHome forever.
pub fn device_identity(router: &toml::Table) -> Result<DeviceIdentity, SetupFailure> {
    // A client name is an arbitrary label, so there is no shape to check
    // beyond it being a string; an address is PARSED, so no unvalidated text
    // can reach a comparison.
    let hostname = read_device_key(router, DeviceKey::Hostname, |text| Some(text.to_string()))?;
    let ipv4 = read_device_key(router, DeviceKey::Ipv4, |text| text.parse().ok())?;
    let mac = read_device_key(router, DeviceKey::Mac, normalized_mac)?;
    if hostname.is_none() && ipv4.is_none() && mac.is_none() {
        return Err(SetupFailure::NoDeviceIdentifier);
    }
    Ok(DeviceIdentity {
        hostname,
        ipv4,
        mac,
    })
}

/// One MAC in the single spelling everything compares in: lowercase, colons,
/// or `None` for six-group text that is not one. THE SAME FUNCTION VALIDATES
/// AND COMPARES, on both sides, so the config's notion of equal and the
/// router's cannot drift apart.
fn normalized_mac(text: &str) -> Option<String> {
    // ONE uniform separator. No separator at all (a bare 12-hex run) and a
    // mix of the two are typos rather than spellings, and accepting the run
    // would mean guessing at a grouping nothing on the wire uses.
    let separator = match (text.contains(':'), text.contains('-')) {
        (true, false) => ':',
        (false, true) => '-',
        _ => return None,
    };
    let groups: Vec<&str> = text.split(separator).collect();
    (groups.len() == 6
        && groups
            .iter()
            .all(|group| group.len() == 2 && group.bytes().all(|byte| byte.is_ascii_hexdigit())))
    .then(|| groups.join(":").to_ascii_lowercase())
}

/// One optional device key, read through its own shape. ABSENT STAYS ABSENT;
/// present but empty, the wrong type, or unreadable as that shape is refused
/// by name. A blank key read as absent is the silent typo the output cannot
/// show: the probe would report having nothing to look for while the
/// operator is looking at the key they filled in.
fn read_device_key<T>(
    router: &toml::Table,
    key: DeviceKey,
    read: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, SetupFailure> {
    let Some(value) = router.get(key.config_key()) else {
        return Ok(None);
    };
    let refuse = || SetupFailure::InvalidDeviceKey {
        key,
        found: spell(value),
    };
    let text = value
        .as_str()
        .filter(|text| !text.is_empty())
        .ok_or_else(refuse)?;
    read(text).map(Some).ok_or_else(refuse)
}

/// One config value as the operator would see it in the file: a string in
/// quotes, anything else by its TOML type, because a non-string has no
/// spelling worth echoing back and its TYPE is what has to change.
fn spell(value: &toml::Value) -> String {
    match value.as_str() {
        Some(text) => format!("{text:?}"),
        None => format!("<{}>", value.type_str()),
    }
}

/// The router's API key out of its own table (`api_key`), or `None` for every
/// way the config can fail to provide one.
///
/// SEPARATE from the settings, and it stays that way: the key never enters a
/// type that derives Debug, so it cannot ride a formatted dump into a log
/// line. Mirrors `moshi_secret`: the path from config to request header must
/// never touch argv, the environment, or an error string.
pub fn router_api_key(router: &toml::Table) -> Option<String> {
    let key = router.get("api_key")?.as_str()?;
    (!key.is_empty()).then(|| key.to_string())
}

/// The hermes route the stale alert posts to, plus the complaint a value that
/// could not be one earns. EMPTY IS THE DEFAULT ROUTE, the same spelling
/// `--channel` and `hermes_url_for` already use, so no second vocabulary for
/// "wherever alerts normally go".
///
/// VALIDATED HERE rather than where the URL is built, because the operator
/// TYPED THIS KEY: `hermes_url_for`'s own refusal names `--channel`, a flag
/// nobody passed on this path, and would send them hunting for it.
///
/// LOUD-WARD ON EVERY FAILURE. A value of the wrong type, an empty string and
/// a name no URL could carry all fall back to the default route with one
/// complaint, because they are one fix; the alert still goes out, on the route
/// the operator would have got had they written nothing. Refusing to alert
/// over a misspelled route would let a config typo silence the very warning it
/// is configuring, and a diagnostic that can be taken down by its own settings
/// is not one.
///
/// THE COMPLAINT IS RETURNED, not printed: this stays a value function, and
/// the composition root decides that a warning goes to stderr, exactly as
/// `select_plugins` hands its roster warning back.
pub fn stale_alert_channel(router: &toml::Table) -> (String, Option<String>) {
    let Some(value) = router.get("stale_alert_channel") else {
        return (String::new(), None);
    };
    match value
        .as_str()
        .filter(|route| crate::channels::hermes::route_name_is_usable(route))
    {
        Some(route) => (route.to_string(), None),
        None => (
            String::new(),
            Some(format!(
                "pns: config error (stale_alert_channel = {} in [plugins.router] is not a \
                 usable route name); the stale alert posts to the default route",
                spell(value)
            )),
        ),
    }
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

/// What `pns home` says for one reading: the verdict, then one EVIDENCE line
/// per configured key, then the staleness warning for whatever the caller
/// hands in as news. PURE, so the words and the reading cannot drift apart
/// untested.
///
/// THE EVIDENCE IS NEVER WITHHELD. A hand-run diagnostic answers "why did it
/// read that" as much as "what did it read", so every configured key says
/// what it found on every run, however many times it has said it before.
///
/// RENDERING ONLY: `news` arrives already decided, because the caller that
/// decides it is also the one that REMEMBERS it. Deriving the episode here
/// as well would settle one fact twice per run off two call sites, and the
/// day either grows a condition (a channel gate, a quiet window) the line
/// the operator read and the episode the file recorded could disagree about
/// what they were told.
pub fn report(reading: &HomeReading, news: Option<&Staleness>) -> String {
    let mut lines = vec![verdict_line(&reading.presence)];
    lines.extend(reading.keys.iter().map(|key| {
        format!(
            "home:   {} {:?} {}",
            key.key.config_key(),
            key.value,
            match &key.outcome {
                // THE CLIENT THE VERDICT NAMES, which is all the scan
                // established. "this device" would claim identity with the
                // operator's own hardware, and when the winning key is
                // itself the stale one (a reclaimed lease answering for a
                // phone that left) the entry it names belongs to somebody
                // else. The evidence surface exists to be read on exactly
                // that reading, so it says what it knows.
                KeyOutcome::MatchedDevice => "matched the client the verdict names".to_string(),
                KeyOutcome::MatchedOtherClient { client } =>
                    format!("matched a different client {client}"),
                KeyOutcome::MatchedNothing => "matched no client".to_string(),
            }
        )
    }));
    // ONLY THE ALERT-SHAPED LINE IS DEDUPED. The evidence above it says the
    // same thing in more words every single run; this one sentence is the
    // one a consumer would act on, so it is said once per state.
    if let Some(staleness) = news {
        lines.push(stale_warning(staleness));
    }
    lines.join("\n")
}

/// The one sentence a stale state is worth saying out loud: which keys
/// disagree, and with which one.
///
/// A FUNCTION OF ITS OWN because it has TWO readers, the terminal line
/// `report` prints and the detail of the alert `pns home` delivers, and a
/// sentence written out twice is a sentence that drifts. Byte-identical in
/// both, deliberately: the operator reading the notification and the operator
/// reading the diagnostic are told the same thing in the same words.
///
/// IT KEEPS THE `home:` PREFIX, which is not only a terminal convention here.
/// The notification's own title says `pns`, and the prefix is what names the
/// PROBE inside it; the alternative was a body that could be about anything
/// this binary does.
///
/// NOTHING THE ROUTER SAID IS IN IT. Every value here is a compiled-in config
/// key name, so no client label, no address and no matched value can ride the
/// sentence out to a channel; the evidence that does carry router text stays
/// in the terminal, escaped by `report`.
pub fn stale_warning(staleness: &Staleness) -> String {
    format!(
        "home: an identifier looks stale: {} {} with {}",
        staleness
            .disagreeing
            .iter()
            .map(|key| key.key.config_key())
            .collect::<Vec<_>>()
            .join(", "),
        if staleness.disagreeing.len() == 1 {
            "disagrees"
        } else {
            "disagree"
        },
        staleness.winner.config_key()
    )
}

/// The one line for the verdict itself. PURE for the same reason as its
/// caller: a swap of the two sentences below survived every suite before
/// this was a function of its own.
fn verdict_line(presence: &HomePresence) -> String {
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

/// Every way the probe can be not set up, as data: the diagnostic's whole
/// vocabulary in one place, each state with its own line, because one
/// "not configured" covering both a missing table and a mistyped value sent
/// the operator to write a table they already had.
#[derive(Debug, PartialEq)]
pub enum SetupFailure {
    NoConfigFile,
    ConfigError(String),
    NoRouterPlugin,
    RouterDisabled,
    NoBrand,
    UnknownBrand(String),
    InvalidRouterTable,
    NoDeviceIdentifier,
    InvalidDeviceKey { key: DeviceKey, found: String },
    NoApiKey,
}

/// The one line for a setup failure. PURE for the same reason as `report`.
pub fn setup_report(failure: &SetupFailure) -> String {
    match failure {
        SetupFailure::NoConfigFile => "home: not configured (no config file)".to_string(),
        SetupFailure::ConfigError(detail) => format!("home: config error ({detail})"),
        SetupFailure::NoRouterPlugin => {
            "home: not configured (no [plugins.router] table)".to_string()
        }
        SetupFailure::RouterDisabled => {
            "home: [plugins.router] is present but enabled = false".to_string()
        }
        SetupFailure::NoBrand => {
            format!("home: no brand in [plugins.router] (the only brand is \"{UNIFI_BRAND}\")")
        }
        SetupFailure::UnknownBrand(brand) => format!(
            "home: [plugins.router] has brand {brand:?}, which no compiled-in backend \
             answers (the only brand is \"{UNIFI_BRAND}\")"
        ),
        SetupFailure::InvalidRouterTable => {
            "home: the [plugins.router] table is present but router_url is missing, empty, \
             or not a string"
                .to_string()
        }
        SetupFailure::NoDeviceIdentifier => format!(
            "home: no device to look for in [plugins.router] (set at least one of {}, {}, {})",
            DeviceKey::Mac.config_key(),
            DeviceKey::Hostname.config_key(),
            DeviceKey::Ipv4.config_key()
        ),
        SetupFailure::InvalidDeviceKey { key, found } => {
            let shape = match key {
                DeviceKey::Mac => {
                    "a MAC address (six hex pairs under one separator, e.g. \"2e:11:ab:6d:b0:4f\")"
                }
                DeviceKey::Hostname => "a client name (a non-empty string)",
                DeviceKey::Ipv4 => "an IPv4 address (a dotted quad, e.g. \"192.168.1.169\")",
            };
            format!(
                "home: {} = {found} in [plugins.router] is not {shape}",
                key.config_key()
            )
        }
        SetupFailure::NoApiKey => {
            "home: no api_key in the [plugins.router] table (the probe is not set up)".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Client, DeviceIdentity, DeviceKey, HomePresence, HomeReading, KeyOutcome, Router,
        RouterSettings, SetupFailure, device_identity, enabled_router_table, episode_id,
        first_site_id, home_reading, is_new_staleness, parse_clients, read_home, report,
        router_api_key, router_settings, setup_report, stale_alert_channel, stale_identifiers,
        stale_warning,
    };

    /// The live capture of 2026-08-20 from the UDR's
    /// `/proxy/network/integration/v1/sites/{id}/clients`, verbatim: four
    /// clients, the phone ("mister") among them with an iOS private MAC.
    const CLIENTS_CAPTURE: &str = r#"{"offset":0,"limit":200,"count":4,"totalCount":4,"data":[{"type":"WIRED","id":"b6363aa8-67c6-326b-93da-90f8e9632d95","name":"hue-bridge-pro","connectedAt":"2026-08-14T03:01:20Z","ipAddress":"192.168.4.37","macAddress":"c4:29:96:bb:6d:cc","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"f7585d92-906f-3c6c-84c3-89e2ce6b2eda","name":"dresden","connectedAt":"2026-08-19T05:44:57Z","ipAddress":"192.168.1.26","macAddress":"3c:06:30:0f:8a:bf","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"71e95d68-f736-3554-b547-75fa1cdc7bf4","name":"mister","connectedAt":"2026-08-20T04:14:55Z","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}},{"type":"WIRELESS","id":"84038944-790a-3965-9f7e-cfee3468308c","name":"mouse","connectedAt":"2026-08-20T05:18:12Z","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01","uplinkDeviceId":"4dd21a37-9e4e-35e8-8b94-5670c43cf93e","access":{"type":"DEFAULT"}}]}"#;

    // --- parsing the router's listing ---------------------------------------

    #[test]
    fn every_client_in_the_live_capture_is_read_with_all_three_of_its_fields() {
        let clients = parse_clients(CLIENTS_CAPTURE).expect("the live capture is a listing");
        assert_eq!(clients.len(), 4);
        assert_eq!(
            clients[2],
            Client {
                name: Some("mister".to_string()),
                ipv4: Some("192.168.1.169".to_string()),
                mac: Some("2e:11:ab:6d:b0:4f".to_string()),
            }
        );
        // An UNNAMED client STAYS in the listing now. The router has not
        // identified it, but it still carries the MAC and the address a
        // configured device can match on, and dropping it would answer
        // NotHome for a device sitting right there in the list.
        assert_eq!(
            parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"},{"name":"mister"}]}"#),
            Some(vec![
                Client {
                    name: None,
                    ipv4: None,
                    mac: Some("2E:11:AB:6D:B0:4F".to_string()),
                },
                Client {
                    name: Some("mister".to_string()),
                    ipv4: None,
                    mac: None,
                },
            ])
        );
    }

    #[test]
    fn a_listing_that_is_not_json_is_no_answer_rather_than_an_empty_wifi() {
        // Unparseable and empty must stay distinct: empty means "nobody is
        // on the wifi" and would read as the phone having LEFT.
        assert_eq!(parse_clients("<html>router login</html>"), None);
        assert_eq!(parse_clients(""), None);
    }

    #[test]
    fn a_json_answer_without_the_data_list_is_no_answer() {
        // The auth-failed shape: valid JSON, no clients in it.
        assert_eq!(parse_clients(r#"{"error":"unauthorized"}"#), None);
        assert_eq!(parse_clients(r#"{"data":"not-a-list"}"#), None);
    }

    #[test]
    fn a_parsed_empty_list_is_an_answer_and_not_a_failure() {
        assert_eq!(
            parse_clients(r#"{"offset":0,"limit":200,"count":0,"totalCount":0,"data":[]}"#),
            Some(Vec::new())
        );
    }

    // --- the verdict ---------------------------------------------------------

    /// A listing of clients the router named and nothing else, which is what
    /// every hostname case is about.
    fn only_names(names: &[&str]) -> Option<Vec<Client>> {
        Some(
            names
                .iter()
                .map(|name| Client {
                    name: Some((*name).to_string()),
                    ipv4: None,
                    mac: None,
                })
                .collect(),
        )
    }

    /// The configured device, through the FRONT DOOR: the identity a test
    /// judges against is the one the operator's table would have produced.
    fn identity(text: &str) -> DeviceIdentity {
        device_identity(&table(text)).expect("a valid device identity")
    }

    #[test]
    fn a_hostname_match_is_exact_so_a_sibling_device_cannot_answer_for_the_phone() {
        let device = identity("device_hostname = \"mister\"\n");
        assert_eq!(
            home_reading(only_names(&["dresden", "mister"]), &device).presence,
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        // A substring match would let "mister-2" answer, and a case-blind
        // one would let "MISTER": both are other devices on this wifi.
        assert_eq!(
            home_reading(only_names(&["mister-2", "MISTER"]), &device).presence,
            HomePresence::NotHome
        );
    }

    #[test]
    fn a_mac_only_identity_reads_home_through_the_one_normalized_spelling() {
        // The operator copied it off a sticker (uppercase, dashes) and the
        // UDR answers lowercase with colons. BOTH SIDES go through the same
        // normalizer, so those are one value and not two.
        let device = identity("device_mac = \"2E-11-AB-6D-B0-4F\"\n");
        let matched = HomePresence::Home {
            matched_by: DeviceKey::Mac,
            value: "2e:11:ab:6d:b0:4f".to_string(),
        };
        assert_eq!(
            home_reading(parse_clients(CLIENTS_CAPTURE), &device).presence,
            matched
        );
        // And an UNNAMED client answers on its MAC, which is what the old
        // name filter would have thrown away.
        assert_eq!(
            home_reading(
                parse_clients(r#"{"data":[{"macAddress":"2E:11:AB:6D:B0:4F"}]}"#),
                &device
            )
            .presence,
            matched
        );
        // A MAC the probe cannot read is a client that matches nothing.
        assert_eq!(
            home_reading(
                parse_clients(r#"{"data":[{"macAddress":"nonsense"},{"name":"dresden"}]}"#),
                &device
            )
            .presence,
            HomePresence::NotHome
        );
    }

    #[test]
    fn an_ipv4_only_identity_reads_home_against_the_client_carrying_that_address() {
        // ADDRESSES are compared, never the texts they were written as: the
        // client's `ipAddress` is parsed the same way the config's value was.
        let device = identity("device_ipv4 = \"192.168.1.169\"\n");
        assert_eq!(
            home_reading(parse_clients(CLIENTS_CAPTURE), &device).presence,
            HomePresence::Home {
                matched_by: DeviceKey::Ipv4,
                value: "192.168.1.169".to_string(),
            }
        );
        // A client whose address is missing or is not an IPv4 is a client
        // that matches nothing. The ROUTER is not the operator: its entries
        // are read for what they hold, never refused for what they lack.
        assert_eq!(
            home_reading(
                parse_clients(r#"{"data":[{"ipAddress":"not-an-address"},{"name":"dresden"}]}"#),
                &device
            )
            .presence,
            HomePresence::NotHome
        );
    }

    /// All three keys pointed at the phone of the live capture, which is the
    /// only shape where the keys can disagree with each other.
    fn full_identity() -> DeviceIdentity {
        identity(
            "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
             device_hostname = \"mister\"\n\
             device_ipv4 = \"192.168.1.169\"\n",
        )
    }

    #[test]
    fn any_one_configured_key_matching_reads_home_while_the_others_match_nothing() {
        // A key that matches NOTHING is skipped, never a failure: the device
        // is home on the strength of the one key that answered, whichever it
        // is. Each listing below carries exactly one of the three.
        for (listing, matched_by, value) in [
            (
                r#"{"data":[{"name":"dresden","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.7"}]}"#,
                DeviceKey::Mac,
                "2e:11:ab:6d:b0:4f",
            ),
            (
                r#"{"data":[{"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"}]}"#,
                DeviceKey::Hostname,
                "mister",
            ),
            (
                r#"{"data":[{"name":"dresden","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.169"}]}"#,
                DeviceKey::Ipv4,
                "192.168.1.169",
            ),
        ] {
            assert_eq!(
                home_reading(parse_clients(listing), &full_identity()).presence,
                HomePresence::Home {
                    matched_by,
                    value: value.to_string(),
                },
                "case: {listing:?}"
            );
        }
    }

    #[test]
    fn on_keys_matching_different_clients_the_verdict_names_the_strongest() {
        // The three keys DISAGREE here: each points at a different client.
        // The strongest one that matched anything is the one that names the
        // device, because a MAC is the device itself, a name is a label the
        // operator can reuse, and an address is only today's lease.
        let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"},
            {"name":"dresden","macAddress":"3c:06:30:0f:8a:bf","ipAddress":"192.168.1.169"}]}"#;
        assert_eq!(
            home_reading(parse_clients(disagreeing), &full_identity()).presence,
            HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            }
        );
        // Drop the MAC and the name is next in line; drop that too and the
        // address answers on its own.
        assert_eq!(
            home_reading(
                parse_clients(disagreeing),
                &identity("device_hostname = \"mister\"\ndevice_ipv4 = \"192.168.1.169\"\n")
            )
            .presence,
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        assert_eq!(
            home_reading(
                parse_clients(disagreeing),
                &identity("device_ipv4 = \"192.168.1.169\"\n")
            )
            .presence,
            HomePresence::Home {
                matched_by: DeviceKey::Ipv4,
                value: "192.168.1.169".to_string(),
            }
        );
    }

    // --- the per-key reading -------------------------------------------------

    #[test]
    fn a_reading_carries_one_entry_per_configured_key_in_precedence_order() {
        let reading = home_reading(parse_clients(CLIENTS_CAPTURE), &full_identity());
        assert_eq!(
            reading
                .keys
                .iter()
                .map(|reading| (reading.key, reading.value.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (DeviceKey::Mac, "2e:11:ab:6d:b0:4f"),
                (DeviceKey::Hostname, "mister"),
                (DeviceKey::Ipv4, "192.168.1.169"),
            ]
        );
        // An UNSET key is skipped rather than reported as absent, and the
        // ORDER is precedence's, not the order the table happened to list
        // them in: this table names the address first.
        let two_keys = home_reading(
            parse_clients(CLIENTS_CAPTURE),
            &identity("device_ipv4 = \"192.168.1.169\"\ndevice_hostname = \"mister\"\n"),
        );
        assert_eq!(
            two_keys
                .keys
                .iter()
                .map(|reading| reading.key)
                .collect::<Vec<_>>(),
            vec![DeviceKey::Hostname, DeviceKey::Ipv4]
        );
    }

    #[test]
    fn every_key_that_found_the_client_the_verdict_names_is_marked_as_this_device() {
        // All three keys point at the phone of the live capture, so all three
        // found the SAME entry: the verdict names the strongest, and no key
        // disagrees with it.
        let reading = home_reading(parse_clients(CLIENTS_CAPTURE), &full_identity());
        assert_eq!(
            reading
                .keys
                .iter()
                .map(|reading| reading.outcome.clone())
                .collect::<Vec<_>>(),
            vec![
                KeyOutcome::MatchedDevice,
                KeyOutcome::MatchedDevice,
                KeyOutcome::MatchedDevice,
            ]
        );
        assert_eq!(
            reading.presence,
            HomePresence::Home {
                matched_by: DeviceKey::Mac,
                value: "2e:11:ab:6d:b0:4f".to_string(),
            }
        );
    }

    #[test]
    fn a_key_that_found_another_client_than_the_verdict_names_says_which_one() {
        // The three keys DISAGREE: the MAC found "mouse" and answers, so the
        // name and the address are pointing at clients this device is not.
        let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"},
            {"name":"dresden","macAddress":"3c:06:30:0f:8a:bf","ipAddress":"192.168.1.169"}]}"#;
        assert_eq!(
            home_reading(parse_clients(disagreeing), &full_identity())
                .keys
                .iter()
                .map(|reading| reading.outcome.clone())
                .collect::<Vec<_>>(),
            vec![
                KeyOutcome::MatchedDevice,
                KeyOutcome::MatchedOtherClient {
                    client: "\"mister\"".to_string(),
                },
                KeyOutcome::MatchedOtherClient {
                    client: "\"dresden\"".to_string(),
                },
            ]
        );
        // The OTHER client is named by the first field that identifies it,
        // its name, then its MAC, then its address: the router has not
        // identified every client it lists, and "a different client" with no
        // way to tell which one is a diagnostic that cannot be acted on.
        for (listing, client) in [
            (
                r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                    {"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
                "\"mouse\"",
            ),
            (
                r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                    {"macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
                "\"60:82:46:3c:fb:01\"",
            ),
            (
                r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                    {"ipAddress":"192.168.1.8"}]}"#,
                "\"192.168.1.8\"",
            ),
            // PRESENT BUT EMPTY is the field the router has no answer for,
            // the same hole as absent: `matched a different client ""` names
            // nothing an operator can act on, so an empty field falls through
            // to the next one exactly as a missing field does.
            (
                r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                    {"name":"","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.8"}]}"#,
                "\"60:82:46:3c:fb:01\"",
            ),
            (
                r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
                    {"name":"","macAddress":"","ipAddress":"192.168.1.8"}]}"#,
                "\"192.168.1.8\"",
            ),
        ] {
            assert_eq!(
                home_reading(
                    parse_clients(listing),
                    &identity("device_hostname = \"mister\"\ndevice_ipv4 = \"192.168.1.8\"\n"),
                )
                .keys
                .last()
                .expect("the address is a configured key")
                .outcome,
                KeyOutcome::MatchedOtherClient {
                    client: client.to_string(),
                },
                "case: {listing:?}"
            );
        }
    }

    #[test]
    fn a_key_the_winners_own_entry_carries_is_this_device_in_either_listing_order() {
        // ONE physical state, listed two ways. Two clients answer to
        // "mister" and only the second-listed one carries the configured MAC,
        // which is the duplicate-entry case the module docs name. The MAC
        // wins either way and its entry ALSO carries the hostname, so nothing
        // disagrees with anything and the order the router happened to list
        // them in cannot change that.
        let winner_second = r#"{"data":[
            {"name":"mister","ipAddress":"192.168.1.7","macAddress":"60:82:46:3c:fb:01"},
            {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"}]}"#;
        let winner_first = r#"{"data":[
            {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
            {"name":"mister","ipAddress":"192.168.1.7","macAddress":"60:82:46:3c:fb:01"}]}"#;
        for listing in [winner_second, winner_first] {
            let reading = home_reading(
                parse_clients(listing),
                &identity(
                    "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                     device_hostname = \"mister\"\n",
                ),
            );
            assert_eq!(
                reading
                    .keys
                    .iter()
                    .map(|reading| reading.outcome.clone())
                    .collect::<Vec<_>>(),
                vec![KeyOutcome::MatchedDevice, KeyOutcome::MatchedDevice],
                "case: {listing:?}"
            );
            assert_eq!(
                stale_identifiers(&reading),
                None,
                "a key the winner's own entry carries is not stale: {listing:?}"
            );
        }
    }

    #[test]
    fn not_home_says_every_key_matched_no_client_and_unknown_says_nothing_at_all() {
        // The NotHome line names no identifier, so the evidence is the only
        // place an operator can see WHICH keys were looked for and that every
        // one of them came up empty.
        let reading = home_reading(
            parse_clients(
                r#"{"totalCount":1,"data":[{"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.248"}]}"#,
            ),
            &full_identity(),
        );
        assert_eq!(reading.presence, HomePresence::NotHome);
        assert_eq!(
            reading
                .keys
                .iter()
                .map(|reading| (reading.key, reading.value.as_str(), reading.outcome.clone()))
                .collect::<Vec<_>>(),
            vec![
                (
                    DeviceKey::Mac,
                    "2e:11:ab:6d:b0:4f",
                    KeyOutcome::MatchedNothing
                ),
                (DeviceKey::Hostname, "mister", KeyOutcome::MatchedNothing),
                (DeviceKey::Ipv4, "192.168.1.169", KeyOutcome::MatchedNothing),
            ]
        );
        // NOTHING WAS SEARCHED for an Unknown, so it carries no readings:
        // "matched no client" is a claim about a listing, and no listing
        // arrived.
        let unknown = home_reading(parse_clients("<html>router login</html>"), &full_identity());
        assert_eq!(unknown.presence, HomePresence::Unknown);
        assert!(unknown.keys.is_empty(), "got: {:?}", unknown.keys);
    }

    #[test]
    fn a_complete_listing_no_key_matched_is_not_home_and_anything_less_is_unknown() {
        let device = full_identity();
        // NotHome needs a COMPLETE listing that none of the three keys found:
        // the wifi answered, and the device was not on it.
        assert_eq!(
            home_reading(
                parse_clients(
                    r#"{"totalCount":1,"data":[{"name":"mouse","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.248"}]}"#
                ),
                &device
            ).presence,
            HomePresence::NotHome
        );
        assert_eq!(
            home_reading(parse_clients(r#"{"totalCount":0,"data":[]}"#), &device).presence,
            HomePresence::NotHome
        );
        // Unreachable, unparseable and INCOMPLETE all stay Unknown. The
        // consumers act on transitions, so inventing a departure out of a
        // page the device could be beyond would fire a false one.
        for no_answer in [
            "<html>router login</html>",
            r#"{"error":"unauthorized"}"#,
            r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"mouse"}]}"#,
        ] {
            assert_eq!(
                home_reading(parse_clients(no_answer), &device).presence,
                HomePresence::Unknown,
                "case: {no_answer:?}"
            );
        }
    }

    // --- the seam, end to end ------------------------------------------------

    struct FakeRouter(Option<&'static str>);
    impl Router for FakeRouter {
        fn clients_json(&self) -> Option<String> {
            self.0.map(str::to_string)
        }
    }

    #[test]
    fn one_reading_runs_fetch_parse_judge_in_order() {
        let device = identity("device_hostname = \"mister\"\n");
        assert_eq!(
            read_home(&FakeRouter(Some(CLIENTS_CAPTURE)), &device).presence,
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        assert_eq!(
            read_home(&FakeRouter(None), &device).presence,
            HomePresence::Unknown
        );
    }

    // --- the settings --------------------------------------------------------

    fn table(text: &str) -> toml::Table {
        text.parse().unwrap()
    }

    #[test]
    fn no_router_plugin_table_at_all_is_not_configured_naming_the_table() {
        // hermes rides along so this is a MISS on the router's own name and
        // not a config the parser dropped whole.
        let config = crate::config::parse_config("[plugins.hermes]\nenabled = true\n").unwrap();
        assert_eq!(
            enabled_router_table(&config),
            Err(SetupFailure::NoRouterPlugin)
        );
        let line = setup_report(&SetupFailure::NoRouterPlugin);
        assert!(line.contains("not configured"), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
    }

    #[test]
    fn a_router_table_switched_off_is_told_apart_from_no_table_at_all() {
        // Selection is the operator's, and a probe they turned off must not
        // read as one they never wrote: the first is fixed by flipping a flag
        // they are looking at, the second by writing a table.
        let config = crate::config::parse_config(
            "[plugins.router]\nenabled = false\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        )
        .unwrap();
        assert_eq!(
            enabled_router_table(&config),
            Err(SetupFailure::RouterDisabled)
        );
        let disabled = setup_report(&SetupFailure::RouterDisabled);
        assert_ne!(disabled, setup_report(&SetupFailure::NoRouterPlugin));
        assert!(disabled.contains("[plugins.router]"), "got: {disabled}");
        assert!(disabled.contains("enabled = false"), "got: {disabled}");
    }

    #[test]
    fn a_router_table_with_no_brand_names_the_key_and_the_one_brand_that_answers() {
        // WHICH ROUTER answers is the first question the table has to settle,
        // because every setting under it belongs to that one. A non-string
        // brand and an EMPTY one are the same hole: there is no name to match
        // a backend by, and the empty one used to be quoted back as a brand
        // nothing implements, which points at a value the operator never
        // typed instead of at the key they left blank.
        for text in [
            "router_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "brand = 5\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "brand = \"\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
        ] {
            assert_eq!(
                router_settings(&table(text)),
                Err(SetupFailure::NoBrand),
                "case: {text:?}"
            );
        }
        let line = setup_report(&SetupFailure::NoBrand);
        assert!(line.contains("brand"), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
        assert!(line.contains("\"unifi\""), "got: {line}");
    }

    #[test]
    fn a_brand_no_compiled_in_backend_answers_is_refused_quoting_it() {
        // Silently probing a UniFi endpoint on a router that is not one would
        // read Unknown forever with nothing to look at; the refusal quotes
        // what was asked for and says what this binary can answer.
        let asus =
            table("brand = \"asus\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n");
        assert_eq!(
            router_settings(&asus),
            Err(SetupFailure::UnknownBrand("asus".to_string()))
        );
        let line = setup_report(&SetupFailure::UnknownBrand("asus".to_string()));
        assert!(line.contains("\"asus\""), "got: {line}");
        assert!(line.contains("\"unifi\""), "got: {line}");
        assert!(line.contains("[plugins.router]"), "got: {line}");
    }

    #[test]
    fn an_enabled_unifi_router_table_yields_its_url_and_device() {
        // The whole value path in one: the config's `[plugins.router]` table,
        // through the selection gate, into the two settings the probe runs on.
        let config = crate::config::parse_config(
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
        )
        .unwrap();
        let router = enabled_router_table(&config).expect("the enabled table");
        assert_eq!(
            router_settings(router),
            Ok(RouterSettings {
                router_url: "https://192.168.1.1".to_string(),
                device: identity("device_hostname = \"mister\"\n"),
            })
        );
    }

    #[test]
    fn a_missing_empty_or_mistyped_url_reports_the_invalid_table_line() {
        // A present-but-wrong VALUE is fixed by editing one line; a missing
        // TABLE is fixed by writing one. `router_url = 5` reported as "no
        // table" used to send the operator to write a table they already had.
        let brand = "brand = \"unifi\"\n";
        for text in [
            "device_hostname = \"mister\"\n",
            "router_url = \"\"\ndevice_hostname = \"mister\"\n",
            "router_url = 5\ndevice_hostname = \"mister\"\n",
        ] {
            assert_eq!(
                router_settings(&table(&format!("{brand}{text}"))),
                Err(SetupFailure::InvalidRouterTable),
                "case: {text:?}"
            );
        }
        let invalid = setup_report(&SetupFailure::InvalidRouterTable);
        assert_ne!(invalid, setup_report(&SetupFailure::NoRouterPlugin));
        assert!(invalid.contains("[plugins.router]"), "got: {invalid}");
        assert!(invalid.contains("router_url"), "got: {invalid}");
        // The line stops naming the device keys: each of the three has its
        // own refusal now, and one covering all four sends the operator to
        // read four keys to find the one that is wrong.
        assert!(!invalid.contains("device_"), "got: {invalid}");
    }

    // --- the device identity -------------------------------------------------

    #[test]
    fn a_router_table_naming_no_device_at_all_is_refused_naming_every_key() {
        // Absent is not configured, and all three absent is a probe with
        // nothing to look for. The line has to spell all three keys: "no
        // device identifier" on its own sends the operator to the docs to
        // find out what one is called.
        assert_eq!(
            device_identity(&table(
                "brand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\n"
            )),
            Err(SetupFailure::NoDeviceIdentifier)
        );
        let line = setup_report(&SetupFailure::NoDeviceIdentifier);
        assert!(line.contains("device_mac"), "got: {line}");
        assert!(line.contains("device_hostname"), "got: {line}");
        assert!(line.contains("device_ipv4"), "got: {line}");
    }

    #[test]
    fn a_table_carrying_only_a_hostname_yields_it_and_the_retired_key_is_not_read() {
        assert_eq!(
            device_identity(&table("device_hostname = \"mister\"\n")),
            Ok(DeviceIdentity {
                hostname: Some("mister".to_string()),
                ipv4: None,
                mac: None,
            })
        );
        // `phone` is the RETIRED spelling. Reading it as an identifier would
        // hide the rename the operator still has to make, and this repo does
        // not carry compatibility for its own unshipped code.
        assert_eq!(
            device_identity(&table("phone = \"mister\"\n")),
            Err(SetupFailure::NoDeviceIdentifier)
        );
        // Present but EMPTY, and present but the wrong TYPE, are the same
        // hole: the key is there and there is no device in it. Read as
        // absent, both would report "no device to look for" while the
        // operator is looking straight at a value they typed.
        // The wrong type is quoted back BY TYPE, because an integer has no
        // spelling worth echoing and the type is what has to change.
        for (text, found) in [
            ("device_hostname = \"\"\n", "\"\""),
            ("device_hostname = 5\n", "<integer>"),
            ("device_hostname = [\"mister\"]\n", "<array>"),
        ] {
            assert_eq!(
                device_identity(&table(text)),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Hostname,
                    found: found.to_string(),
                }),
                "case: {text:?}"
            );
        }
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Hostname,
            found: "<integer>".to_string(),
        });
        assert!(line.contains("device_hostname"), "got: {line}");
        assert!(line.contains("<integer>"), "got: {line}");
    }

    #[test]
    fn a_malformed_device_ipv4_is_refused_naming_the_key_and_quoting_the_value() {
        // The stdlib parser IS the validator, and these are the shapes it
        // refuses, measured on this toolchain (rustc 1.92.0-nightly): a
        // leading zero, a short quad, a long one, an octet past 255, and
        // surrounding whitespace. A hand-rolled octet parser would have to
        // relearn every one of them.
        for bad in [
            "010.1.1.1",
            "1.2.3",
            "1.2.3.4.5",
            "256.1.1.1",
            " 1.2.3.4",
            "1.2.3.4 ",
            "",
        ] {
            assert_eq!(
                device_identity(&table(&format!("device_ipv4 = \"{bad}\"\n"))),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Ipv4,
                    found: format!("{bad:?}"),
                }),
                "case: {bad:?}"
            );
        }
        // The line quotes what was typed, so the typo is visible without
        // opening the file.
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Ipv4,
            found: "\"1.2.3\"".to_string(),
        });
        assert!(line.contains("device_ipv4"), "got: {line}");
        assert!(line.contains("\"1.2.3\""), "got: {line}");
        // A well-formed one is kept as an ADDRESS, never as the text it was
        // typed as: nothing downstream can compare it as a string.
        assert_eq!(
            device_identity(&table("device_ipv4 = \"192.168.1.169\"\n")),
            Ok(DeviceIdentity {
                hostname: None,
                ipv4: Some(std::net::Ipv4Addr::new(192, 168, 1, 169)),
                mac: None,
            })
        );
    }

    #[test]
    fn a_malformed_device_mac_is_refused_naming_the_key_and_quoting_the_value() {
        // Too few groups, too many, a bare 12-hex run, a non-hex digit, a
        // group that is not exactly two digits, a MIXED separator, and a
        // trailing one. A single uniform separator is what tells a MAC from
        // a typo, and accepting the bare run would mean guessing at
        // groupings the router never uses.
        for bad in [
            "2e:11:ab:6d:b0",
            "2e:11:ab:6d:b0:4f:aa",
            "2e11ab6db04f",
            "zz:11:ab:6d:b0:4f",
            "2e:1:ab:6d:b0:4f",
            "2e-11:ab-6d:b0-4f",
            "2e:11:ab:6d:b0:4f:",
        ] {
            assert_eq!(
                device_identity(&table(&format!("device_mac = \"{bad}\"\n"))),
                Err(SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Mac,
                    found: format!("{bad:?}"),
                }),
                "case: {bad:?}"
            );
        }
        let line = setup_report(&SetupFailure::InvalidDeviceKey {
            key: DeviceKey::Mac,
            found: "\"2e11ab6db04f\"".to_string(),
        });
        assert!(line.contains("device_mac"), "got: {line}");
        assert!(line.contains("\"2e11ab6db04f\""), "got: {line}");
    }

    #[test]
    fn a_well_formed_mac_in_any_case_or_separator_validates_to_one_spelling() {
        // ONE spelling is what makes the two sides comparable at all: the
        // operator may copy the MAC off a sticker in uppercase with dashes,
        // and the UDR answers in lowercase with colons.
        for typed in [
            "2e:11:ab:6d:b0:4f",
            "2E:11:AB:6D:B0:4F",
            "2e-11-ab-6d-b0-4f",
            "2E-11-AB-6D-B0-4F",
        ] {
            assert_eq!(
                device_identity(&table(&format!("device_mac = \"{typed}\"\n"))),
                Ok(DeviceIdentity {
                    hostname: None,
                    ipv4: None,
                    mac: Some("2e:11:ab:6d:b0:4f".to_string()),
                }),
                "case: {typed:?}"
            );
        }
    }

    // --- the secret ----------------------------------------------------------

    #[test]
    fn the_api_key_reads_from_the_router_plugin_table_beside_the_settings() {
        // Through the config, so this pins WHERE the key is read from and not
        // only how: a table lifted from anywhere else would pass a bare-table
        // assertion just as well.
        let config = crate::config::parse_config(
            "[plugins.router]\nenabled = true\nbrand = \"unifi\"\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\napi_key = \"k-123\"\n",
        )
        .unwrap();
        let router = enabled_router_table(&config).expect("the enabled table");
        assert_eq!(router_api_key(router), Some("k-123".to_string()));
    }

    #[test]
    fn every_way_the_router_table_fails_to_provide_a_key_is_quietly_not_set_up() {
        for router in ["", "api_key = \"\"\n", "api_key = 5\n"] {
            assert_eq!(router_api_key(&table(router)), None, "case: {router:?}");
        }
        // And the line sends the operator to the table the key now lives in.
        let line = setup_report(&SetupFailure::NoApiKey);
        assert!(line.contains("[plugins.router]"), "got: {line}");
        assert!(line.contains("api_key"), "got: {line}");
    }

    // --- where the stale alert is routed -------------------------------------

    #[test]
    fn a_usable_stale_alert_channel_is_read_back_as_the_route_verbatim() {
        // The operator names a hermes ROUTE, and the name they typed is what
        // the alert carries: nothing here rewrites, lowercases or defaults it.
        assert_eq!(
            stale_alert_channel(&table("stale_alert_channel = \"priority\"\n")),
            ("priority".to_string(), None)
        );
    }

    #[test]
    fn no_stale_alert_channel_at_all_asks_for_the_default_route_in_silence() {
        // ABSENT IS NOT AN ERROR: the key is optional, and an empty route is
        // how every caller of `hermes_url_for` spells "the default alert
        // route". Complaining here would put a config error in front of every
        // operator who never asked to route the alert anywhere.
        assert_eq!(
            stale_alert_channel(&table("brand = \"unifi\"\n")),
            (String::new(), None)
        );
    }

    #[test]
    fn a_stale_alert_channel_that_is_not_a_usable_route_complains_and_falls_back() {
        // LOUD-WARD, the same direction `hermes_url_for` falls: a misrouted
        // alert on the default route beats one silently dropped, and the
        // complaint names the CONFIG KEY, because the config is where the
        // operator has to go. The three ways the value can fail are one
        // message, because they are one fix.
        for (setting, quoted) in [
            ("stale_alert_channel = \"\"\n", "\"\""),
            ("stale_alert_channel = \"a/b\"\n", "\"a/b\""),
            ("stale_alert_channel = \"../alert\"\n", "\"../alert\""),
            ("stale_alert_channel = 5\n", "<integer>"),
            ("stale_alert_channel = true\n", "<boolean>"),
        ] {
            let (route, complaint) = stale_alert_channel(&table(setting));
            assert_eq!(route, String::new(), "case: {setting:?}");
            assert_eq!(
                complaint.as_deref(),
                Some(
                    format!(
                        "pns: config error (stale_alert_channel = {quoted} in [plugins.router] \
                         is not a usable route name); the stale alert posts to the default route"
                    )
                    .as_str()
                ),
                "case: {setting:?}"
            );
        }
    }

    // --- pagination: an incomplete page must not report a departure ----------

    #[test]
    fn a_page_the_phone_could_be_beyond_is_no_answer_rather_than_not_home() {
        // totalCount says 201 clients exist and the page carries one: the
        // phone may be among the 200 not shown, so absent-from-this-page must
        // not become NotHome, which is a false departure.
        assert_eq!(
            parse_clients(
                r#"{"offset":0,"limit":200,"count":1,"totalCount":201,"data":[{"name":"dresden"}]}"#
            ),
            None
        );
    }

    #[test]
    fn a_complete_page_with_unnamed_clients_still_answers() {
        // Completeness is judged on the ENTRIES: a page carrying as many as
        // the router counted is whole, and the unnamed entry is one of them.
        assert_eq!(
            parse_clients(r#"{"totalCount":2,"data":[{"id":"x"},{"name":"mister"}]}"#)
                .map(|clients| clients.len()),
            Some(2)
        );
    }

    #[test]
    fn a_listing_without_total_count_is_taken_whole_as_before() {
        assert_eq!(
            parse_clients(r#"{"data":[{"name":"mister"}]}"#),
            only_names(&["mister"])
        );
    }

    // --- the site id, the one router answer that becomes part of a URL -------

    #[test]
    fn the_first_sites_id_is_extracted_from_the_live_shape() {
        assert_eq!(
            first_site_id(
                r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#
            ),
            Some("88f7af54-98f8-306a-a1c7-c9349722b1f6".to_string())
        );
    }

    #[test]
    fn a_sites_answer_without_a_usable_id_is_no_answer() {
        for sites in [
            "",
            "not json",
            r#"{"data":[]}"#,
            r#"{"data":[{}]}"#,
            r#"{"data":[{"id":5}]}"#,
        ] {
            assert_eq!(first_site_id(sites), None, "case: {sites:?}");
        }
    }

    #[test]
    fn an_id_that_could_escape_the_url_path_is_refused_outright() {
        // The id is joined into /sites/{id}/clients, so this is the trust
        // boundary: a corrupt or hostile router answer must never become a
        // path segment.
        for hostile in ["../evil", "a/b", "a?x=1", "a#frag", "id with space", ""] {
            assert_eq!(
                first_site_id(&format!(r#"{{"data":[{{"id":"{hostile}"}}]}}"#)),
                None,
                "case: {hostile:?}"
            );
        }
    }

    // --- the staleness ------------------------------------------------------

    #[test]
    fn a_staleness_is_a_home_verdict_with_a_key_pointing_somewhere_else() {
        // The MAC found "mouse" and answers; the name found the OTHER client
        // and the address found nobody. Both of those disagree with the
        // winner, which is the whole state this detects.
        let disagreeing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#;
        let staleness =
            stale_identifiers(&home_reading(parse_clients(disagreeing), &full_identity()))
                .expect("two keys point away from the client the MAC named");
        assert_eq!(staleness.winner, DeviceKey::Mac);
        assert_eq!(
            staleness
                .disagreeing
                .iter()
                .map(|reading| (reading.key, reading.outcome.clone()))
                .collect::<Vec<_>>(),
            vec![
                (
                    DeviceKey::Hostname,
                    KeyOutcome::MatchedOtherClient {
                        client: "\"mister\"".to_string(),
                    }
                ),
                (DeviceKey::Ipv4, KeyOutcome::MatchedNothing),
            ]
        );
        // KEYS THAT AGREE are not a staleness, ONE key has nothing to
        // disagree with, and away is not stale: every key matching nothing is
        // what NotHome IS, and an Unknown searched nothing at all.
        for (listing, device, case) in [
            (
                CLIENTS_CAPTURE,
                full_identity(),
                "every key found the phone",
            ),
            (
                r#"{"data":[{"name":"mister"}]}"#,
                identity("device_hostname = \"mister\"\n"),
                "one configured key",
            ),
            (
                r#"{"totalCount":1,"data":[{"name":"mouse"}]}"#,
                full_identity(),
                "not home",
            ),
            ("<html>router login</html>", full_identity(), "unknown"),
        ] {
            assert_eq!(
                stale_identifiers(&home_reading(parse_clients(listing), &device)),
                None,
                "case: {case}"
            );
        }
    }

    /// The staleness in one listing judged against one table, which is the
    /// only way an episode identity is ever spelled.
    fn episode(listing: &str, device: &str) -> String {
        episode_id(
            &stale_identifiers(&home_reading(parse_clients(listing), &identity(device)))
                .expect("a staleness"),
        )
    }

    #[test]
    fn an_episode_identity_spells_the_state_and_never_the_values_that_moved() {
        // The MAC answers, the name is pointing at another client and the
        // address is pointing at nobody: THAT is the state, and its spelling
        // is the winner plus each disagreeing key and what it found.
        assert_eq!(
            episode(
                r#"{"data":[
                    {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
                    {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#,
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister\"\n\
                 device_ipv4 = \"192.168.1.169\"\n",
            ),
            "device_mac device_hostname=other device_ipv4=none"
        );
        // DHCP CHURN IS NOT NEWS. Every value here moved (a different stale
        // address, a different client under the name, a different label on
        // it) while the state did not, so the operator is not told the same
        // thing twice.
        assert_eq!(
            episode(
                r#"{"data":[
                    {"name":"kite","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.9.9"},
                    {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#,
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"kite\"\n\
                 device_ipv4 = \"10.0.0.5\"\n",
            ),
            "device_mac device_hostname=other device_ipv4=none"
        );
    }

    #[test]
    fn a_changed_stale_set_outcome_or_winner_each_spell_a_different_identity() {
        // One listing, four ways the STATE can differ under it. The MAC and
        // the name both point at "mouse" in the first case, so only the
        // address disagrees.
        let listing = r#"{"data":[
            {"name":"mister","macAddress":"60:82:46:3c:fb:01","ipAddress":"192.168.1.7"},
            {"name":"mouse","macAddress":"2e:11:ab:6d:b0:4f","ipAddress":"192.168.1.8"}]}"#;
        let mut identities = vec![
            episode(
                listing,
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mouse\"\n\
                 device_ipv4 = \"192.168.1.169\"\n",
            ),
            // A DIFFERENT KEY ANSWERS, over the same one stale address: the
            // operator is now home on the strength of a label rather than the
            // hardware, which is a weaker footing and its own news.
            episode(
                listing,
                "device_hostname = \"mouse\"\n\
                 device_ipv4 = \"192.168.1.169\"\n",
            ),
            // The address STOPPED matching nothing and started matching
            // somebody else.
            episode(
                listing,
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mouse\"\n\
                 device_ipv4 = \"192.168.1.7\"\n",
            ),
            // The name JOINED the stale set.
            episode(
                listing,
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister\"\n\
                 device_ipv4 = \"192.168.1.169\"\n",
            ),
        ];
        assert_eq!(
            identities,
            vec![
                "device_mac device_ipv4=none",
                "device_hostname device_ipv4=none",
                "device_mac device_ipv4=other",
                "device_mac device_hostname=other device_ipv4=none",
            ]
        );
        identities.sort();
        identities.dedup();
        assert_eq!(identities.len(), 4, "each state is its own news");
    }

    #[test]
    fn a_staleness_is_news_only_when_its_identity_differs_from_the_remembered_one() {
        // The dedupe is over VALUES, not over readings: the same state read
        // fifty times is one piece of news, and a state that RESOLVED is news
        // to nobody, because the operator was told about a disagreement that
        // is no longer there.
        for (remembered, current, news, case) in [
            (
                None,
                Some("device_mac device_ipv4=none"),
                true,
                "first sighting",
            ),
            (
                Some("device_mac device_ipv4=none"),
                Some("device_mac device_ipv4=none"),
                false,
                "the same state again",
            ),
            (
                Some("device_mac device_ipv4=none"),
                Some("device_mac device_ipv4=other"),
                true,
                "the state moved",
            ),
            (
                Some("device_mac device_ipv4=none"),
                None,
                false,
                "the disagreement resolved",
            ),
            (None, None, false, "nothing to say"),
        ] {
            assert_eq!(is_new_staleness(remembered, current), news, "case: {case}");
        }
    }

    #[test]
    fn the_stale_warning_is_one_sentence_that_agrees_with_the_keys_it_names() {
        // THE SENTENCE A CONSUMER ACTS ON, which is why it is a function of
        // its own: the diagnostic prints it and the delivered alert carries
        // it, and one spelling is what keeps the terminal line and the
        // notification from drifting apart.
        let two_disagree = home_reading(
            parse_clients(CLIENTS_CAPTURE),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister-2\"\n\
                 device_ipv4 = \"192.168.1.248\"\n",
            ),
        );
        assert_eq!(
            stale_warning(&stale_identifiers(&two_disagree).expect("two keys point away")),
            "home: an identifier looks stale: device_hostname, device_ipv4 \
             disagree with device_mac"
        );
        // ONE disagreeing key is one key: the verb agrees with what it names.
        let one_disagrees = home_reading(
            parse_clients(CLIENTS_CAPTURE),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_ipv4 = \"192.168.1.248\"\n",
            ),
        );
        assert_eq!(
            stale_warning(&stale_identifiers(&one_disagrees).expect("one key points away")),
            "home: an identifier looks stale: device_ipv4 disagrees with device_mac"
        );
    }

    // --- the reported lines, pinned so the words match the verdict -----------

    /// A verdict with no evidence under it, which is what an Unknown really
    /// is: the verdict LINE is what these cases pin, one sentence at a time.
    fn verdict_only(presence: HomePresence) -> HomeReading {
        HomeReading {
            presence,
            keys: Vec::new(),
        }
    }

    #[test]
    fn each_presence_verdict_reports_its_own_line() {
        // The Home line NAMES the identifier that answered and the value it
        // answered with, which is the only observable difference precedence
        // makes: the operator can see WHICH key spoke without a per-key
        // breakdown that would expose the probe's internals.
        assert_eq!(
            report(
                &verdict_only(HomePresence::Home {
                    matched_by: DeviceKey::Mac,
                    value: "2e:11:ab:6d:b0:4f".to_string(),
                }),
                None
            ),
            "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")"
        );
        assert_eq!(
            report(
                &verdict_only(HomePresence::Home {
                    matched_by: DeviceKey::Hostname,
                    value: "mister".to_string(),
                }),
                None
            ),
            "home: on the home network (matched by device_hostname \"mister\")"
        );
        // THE VALUE IS ESCAPED, exactly as `spell` escapes a config value next
        // door: a client name carrying a quote or an ESC byte reaches stdout as
        // its escape, never as the byte. The two lines above are the proof this
        // costs nothing to read: debug-quoting a plain string is the same
        // quoted form it always had.
        assert_eq!(
            report(
                &verdict_only(HomePresence::Home {
                    matched_by: DeviceKey::Hostname,
                    value: "mist\"er\u{1b}[2J".to_string(),
                }),
                None
            ),
            "home: on the home network (matched by device_hostname \"mist\\\"er\\u{1b}[2J\")"
        );
        assert_eq!(
            report(&verdict_only(HomePresence::NotHome), None),
            "home: NOT on the home network (no configured identifier matched a client)"
        );
        assert_eq!(
            report(&verdict_only(HomePresence::Unknown), None),
            "home: unknown (router unreachable or its answer unreadable)"
        );
    }

    #[test]
    fn the_evidence_under_the_verdict_says_what_each_key_found_escaping_the_label() {
        // Every CONFIGURED key gets a line, whatever it found, because the
        // diagnostic's job is to show the disagreement rather than the
        // winner. The ROUTER is not the operator: the client label is the one
        // string on these lines nobody here typed, so it reaches a terminal
        // as its escape exactly as the matched value does.
        let listing = r#"{"data":[{"name":"mister","ipAddress":"192.168.1.7"},
            {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.8"}]}"#;
        let reading = home_reading(
            parse_clients(listing),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister\"\n\
                 device_ipv4 = \"192.168.1.8\"\n",
            ),
        );
        assert_eq!(
            report(&reading, stale_identifiers(&reading).as_ref()),
            "home: on the home network (matched by device_hostname \"mister\")\n\
             home:   device_mac \"2e:11:ab:6d:b0:4f\" matched no client\n\
             home:   device_hostname \"mister\" matched the client the verdict names\n\
             home:   device_ipv4 \"192.168.1.8\" matched a different client \
             \"mo\\\"use\\u{1b}[2J\"\n\
             home: an identifier looks stale: device_mac, device_ipv4 disagree with \
             device_hostname"
        );
    }

    #[test]
    fn the_staleness_line_names_the_disagreeing_keys_and_prints_only_when_it_is_news() {
        // The operator's own case: the MAC still names the phone, the name
        // key has gone stale against a client that left, and the address is
        // now somebody else's lease.
        let reading = home_reading(
            parse_clients(CLIENTS_CAPTURE),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_hostname = \"mister-2\"\n\
                 device_ipv4 = \"192.168.1.248\"\n",
            ),
        );
        let evidence = "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
             home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
             home:   device_hostname \"mister-2\" matched no client\n\
             home:   device_ipv4 \"192.168.1.248\" matched a different client \"mouse\"";
        assert_eq!(
            report(&reading, stale_identifiers(&reading).as_ref()),
            format!(
                "{evidence}\nhome: an identifier looks stale: device_hostname, device_ipv4 \
                 disagree with device_mac"
            )
        );
        // A REPEAT keeps every evidence line and drops the alert-shaped one:
        // a hand-run diagnostic always tells the whole truth, and only the
        // warning is said once.
        assert_eq!(report(&reading, None), evidence);
        // ONE disagreeing key is one key: the sentence agrees with what it
        // is naming.
        let one_key = home_reading(
            parse_clients(CLIENTS_CAPTURE),
            &identity(
                "device_mac = \"2e:11:ab:6d:b0:4f\"\n\
                 device_ipv4 = \"192.168.1.248\"\n",
            ),
        );
        assert_eq!(
            report(&one_key, stale_identifiers(&one_key).as_ref())
                .lines()
                .last()
                .expect("a staleness line"),
            "home: an identifier looks stale: device_ipv4 disagrees with device_mac"
        );
    }

    #[test]
    fn every_setup_failure_line_names_what_to_look_at() {
        for (failure, needle) in [
            (SetupFailure::NoConfigFile, "no config file"),
            (
                SetupFailure::ConfigError("bad at line 3".to_string()),
                "bad at line 3",
            ),
            (SetupFailure::NoRouterPlugin, "[plugins.router]"),
            (SetupFailure::RouterDisabled, "[plugins.router]"),
            (SetupFailure::NoBrand, "brand"),
            (SetupFailure::UnknownBrand("asus".to_string()), "asus"),
            (SetupFailure::InvalidRouterTable, "router_url"),
            (SetupFailure::NoDeviceIdentifier, "device_hostname"),
            (
                SetupFailure::InvalidDeviceKey {
                    key: DeviceKey::Ipv4,
                    found: "\"1.2.3\"".to_string(),
                },
                "device_ipv4",
            ),
            (SetupFailure::NoApiKey, "api_key"),
        ] {
            let line = setup_report(&failure);
            assert!(line.starts_with("home: "), "case {failure:?}: {line}");
            assert!(line.contains(needle), "case {failure:?}: {line}");
        }
    }

    // --- the production adapter, over a scripted transport -------------------
    //
    // The URLProtocolStub move, in Rust: ureq's `Agent::with_parts` accepts a
    // bespoke `Connector`, so the REAL agent pipeline runs (URL building, the
    // header, redirect policy, the body cap) and only the wire is scripted.
    // The seam lives in `ureq::unversioned`, which is exempt from semver;
    // Cargo.lock pins the version, so it can only shift the day ureq is
    // deliberately bumped, with these tests here to catch it.

    use std::sync::{Arc, Mutex};
    use ureq::unversioned::resolver::DefaultResolver;
    use ureq::unversioned::transport::{
        Buffers, ConnectionDetails, Connector, LazyBuffers, NextTimeout, Transport,
    };

    /// Hands out one scripted response per connection and keeps a shared
    /// capture of every byte the adapter transmits.
    #[derive(Debug, Default)]
    struct ScriptedConnector {
        wire: Arc<Mutex<Vec<u8>>>,
        responses: Arc<Mutex<std::collections::VecDeque<Vec<u8>>>>,
    }

    impl Connector for ScriptedConnector {
        type Out = ScriptedTransport;

        fn connect(
            &self,
            _details: &ConnectionDetails,
            _chained: Option<()>,
        ) -> Result<Option<Self::Out>, ureq::Error> {
            let response = self
                .responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_default();
            Ok(Some(ScriptedTransport {
                buffers: LazyBuffers::new(65536, 65536),
                wire: Arc::clone(&self.wire),
                response,
                fed: 0,
            }))
        }
    }

    /// One connection: records what is transmitted, feeds the scripted
    /// response in chunks, and refuses reuse so every request reconnects and
    /// pops the next script entry.
    #[derive(Debug)]
    struct ScriptedTransport {
        buffers: LazyBuffers,
        wire: Arc<Mutex<Vec<u8>>>,
        response: Vec<u8>,
        fed: usize,
    }

    impl Transport for ScriptedTransport {
        fn buffers(&mut self) -> &mut dyn Buffers {
            &mut self.buffers
        }

        fn transmit_output(
            &mut self,
            amount: usize,
            _timeout: NextTimeout,
        ) -> Result<(), ureq::Error> {
            let sent = self.buffers.output()[..amount].to_vec();
            self.wire.lock().unwrap().extend_from_slice(&sent);
            Ok(())
        }

        fn await_input(&mut self, _timeout: NextTimeout) -> Result<bool, ureq::Error> {
            let pending = &self.response[self.fed..];
            if pending.is_empty() {
                return Ok(false);
            }
            let sink = self.buffers.input_append_buf();
            let amount = pending.len().min(sink.len());
            sink[..amount].copy_from_slice(&pending[..amount]);
            self.buffers.input_appended(amount);
            self.fed += amount;
            Ok(amount > 0)
        }

        fn is_open(&mut self) -> bool {
            // A finished response closes the connection, so the agent cannot
            // pool it: the next request reconnects and pops the next script.
            self.fed < self.response.len()
        }
    }

    /// The production config semantics (no redirects) over the scripted wire.
    fn scripted_router(
        responses: &[Vec<u8>],
        key: &str,
    ) -> (super::UniFiRouter, Arc<Mutex<Vec<u8>>>) {
        let connector = ScriptedConnector::default();
        let wire = Arc::clone(&connector.wire);
        *connector.responses.lock().unwrap() = responses.iter().cloned().collect();
        let config = ureq::Agent::config_builder().max_redirects(0).build();
        let agent = ureq::Agent::with_parts(config, connector, DefaultResolver::default());
        (
            super::UniFiRouter::with_agent(
                agent,
                "http://localhost:9".to_string(),
                key.to_string(),
            ),
            wire,
        )
    }

    fn http_ok(body: &str) -> Vec<u8> {
        format!(
            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len()
        )
        .into_bytes()
    }

    const SITES_CAPTURE: &str = r#"{"offset":0,"limit":25,"count":1,"totalCount":1,"data":[{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6","internalReference":"default","name":"Default"}]}"#;

    #[test]
    fn the_adapter_sends_the_key_and_walks_sites_then_clients() {
        let (router, wire) =
            scripted_router(&[http_ok(SITES_CAPTURE), http_ok(CLIENTS_CAPTURE)], "k-123");
        assert_eq!(
            read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
            HomePresence::Home {
                matched_by: DeviceKey::Hostname,
                value: "mister".to_string(),
            }
        );
        let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
        let sites_at = wire
            .find("get /proxy/network/integration/v1/sites http/1.1")
            .expect("the sites request was made");
        let clients_at = wire
            .find(
                "get /proxy/network/integration/v1/sites/88f7af54-98f8-306a-a1c7-c9349722b1f6/clients?limit=200 http/1.1",
            )
            .expect("the clients request was made with the extracted site id");
        assert!(sites_at < clients_at, "sites is asked before clients");
        assert!(
            wire.contains("x-api-key: k-123"),
            "the key rides the header: {wire}"
        );
    }

    #[test]
    fn a_redirecting_router_is_never_followed_and_reads_unknown() {
        // max_redirects(0) is production config: a router answering 301 must
        // not send the adapter (and its key header) to the Location target.
        let (router, wire) = scripted_router(
            &[
                b"HTTP/1.1 301 Moved Permanently\r\nlocation: http://elsewhere.test/\r\ncontent-length: 0\r\n\r\n".to_vec(),
            ],
            "k-123",
        );
        assert_eq!(
            read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
            HomePresence::Unknown
        );
        let wire = String::from_utf8_lossy(&wire.lock().unwrap()).to_lowercase();
        assert_eq!(wire.matches("get ").count(), 1, "no second request: {wire}");
        assert!(
            !wire.contains("elsewhere"),
            "the redirect target is never contacted"
        );
    }

    #[test]
    fn a_body_past_the_cap_reads_unknown_rather_than_being_swallowed() {
        // The oversized answer is VALID and would parse into Home if it were
        // read: that is what makes this pin the 1MB cap itself rather than
        // ureq's own 10MB default, which an unparseable body cannot tell
        // apart (a garbage body reads Unknown under any cap).
        let oversized_but_valid_sites = format!(
            r#"{{"padding":"{}","data":[{{"id":"88f7af54-98f8-306a-a1c7-c9349722b1f6"}}]}}"#,
            "x".repeat(1_100_000)
        );
        let (router, _wire) = scripted_router(
            &[
                http_ok(&oversized_but_valid_sites),
                http_ok(CLIENTS_CAPTURE),
            ],
            "k-123",
        );
        assert_eq!(
            read_home(&router, &identity("device_hostname = \"mister\"\n")).presence,
            HomePresence::Unknown
        );
    }

    #[test]
    fn an_unknown_brand_with_control_bytes_is_escaped_like_every_other_spelled_value() {
        let line = setup_report(&SetupFailure::UnknownBrand("a\u{1b}[31mz".to_string()));
        assert!(
            !line.contains('\u{1b}'),
            "raw ESC must not reach stdout: {line}"
        );
        assert!(
            line.contains("\\u{1b}"),
            "the escaped form is shown: {line}"
        );
    }
}
