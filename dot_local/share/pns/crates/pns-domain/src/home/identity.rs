//! Which device a key names, and what a client on the network is called.

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
    pub hostname: Option<String>,
    pub ipv4: Option<std::net::Ipv4Addr>,
    pub mac: Option<String>,
}
/// One client the router listed, in the router's own spelling. Every field is
/// optional because every one of them is: the UDR omits `name` for a client
/// it has not identified, and a client can be listed before it has an
/// address. Nothing here is validated, because the router is not the operator:
/// an entry this probe cannot read is a client that matches nothing, never a
/// listing that failed.
#[derive(Debug, PartialEq)]
pub struct Client {
    pub name: Option<String>,
    pub ipv4: Option<String>,
    pub mac: Option<String>,
}
/// Whether ONE listed client carries the value of ONE configured key: the
/// single implementation of "this key matches that entry", asked to find the
/// verdict and asked again of the entry the verdict names.
pub(super) fn client_carries(client: &Client, key: DeviceKey, device: &DeviceIdentity) -> bool {
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
/// The client a disagreeing key found, as the operator can recognize it:
/// the first field that identifies it, its name, then its MAC, then its
/// address. SPELLED here rather than at the print, the way
/// `SetupFailure::InvalidDeviceKey`'s `found` is, so the router's own text
/// reaches a terminal as its escape and the nameless case reads as prose.
///
/// PRESENT BUT EMPTY IS NOT IDENTIFYING, the same filter `router_settings`
/// puts on a blank `type`: the router lists a field it has no answer for as
/// `""` as readily as it omits it, and `matched a different client ""` names
/// nothing the operator can act on.
///
/// A client a key MATCHED always carries at least the field that matched it,
/// so the last arm is the floor of a total function rather than a case the
/// router produces.
pub(super) fn client_label(client: &Client) -> String {
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
/// The one type a compiled-in backend answers. It is VALIDATED and then
/// discarded: `trait Router` is the seam a second backend enters through, and
/// the enum that dispatches between two of them is worth writing the day
/// there are two.
///
/// PUBLIC FOR THE DIAGNOSTIC ALONE. `pns doctor` names it when it warns about
/// a router table that is switched off and names no backend, which is the one
/// place that misconfiguration is visible at all.
pub const UNIFI_TYPE: &str = "unifi";
/// One MAC in the single spelling everything compares in: lowercase, colons,
/// or `None` for six-group text that is not one. THE SAME FUNCTION VALIDATES
/// AND COMPARES, on both sides, so the config's notion of equal and the
/// router's cannot drift apart.
pub fn normalized_mac(text: &str) -> Option<String> {
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
