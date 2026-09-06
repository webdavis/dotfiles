//! Whether the phone is home, from the keys and the clients that matched.

use super::identity::{Client, DeviceIdentity, DeviceKey, client_carries, client_label};

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

/// One reading of the listing: the verdict, plus what EVERY configured key
/// found on the way to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeReading {
    pub presence: HomePresence,
    pub keys: Vec<KeyReading>,
}
