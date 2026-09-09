//! A key that points at nothing, and the episode it belongs to.

use super::identity::DeviceKey;
use super::reading::{HomePresence, HomeReading, KeyOutcome, KeyReading};

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
