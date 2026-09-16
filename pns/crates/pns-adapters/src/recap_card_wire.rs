//! The one line a return moment hands its recap child: the card that process
//! is now responsible for dispatching.
//!
//! IT IS AN INTERNAL HAND-OFF AND NOT A PUBLIC CONTRACT, which is why it lives
//! here and not in `pns-protocol`. Both ends are the same binary, reached
//! through `current_exe`, so there is no version to negotiate and nothing
//! third-party compiles against it. What it does share with every other wire
//! here is that JSON is not hand-parsed.
//!
//! NOTHING SECRET AND NOTHING ON ARGV. The card carries the operator's own
//! missed-notification sentence, which belongs on a pipe rather than in a
//! world-readable process table, for the same reason the moshi token does.

use pns_application::{ReplayCard, SubmissionIdentity};
use pns_domain::routing::{Leg, ReportMode};

/// The card as the child reads it back: owned, and with its legs resolved to
/// registered destinations.
pub struct HandedCard {
    pub identity: SubmissionIdentity,
    pub detail: String,
    pub legs: Vec<Leg>,
}

pub fn encode(card: &ReplayCard<'_>) -> String {
    serde_json::json!({
        "producer": card.identity.producer,
        "request_id": card.identity.request_id,
        "detail": card.detail,
        "legs": card
            .legs
            .iter()
            .map(|leg| serde_json::json!({
                "name": leg.name,
                "mode": leg.mode.as_str(),
                "decorative": leg.decorative,
            }))
            .collect::<Vec<_>>(),
    })
    .to_string()
}

/// The card, or nothing at all.
///
/// AN EMPTY PIPE IS NO CARD rather than a fault: the return moment closes the
/// pipe without writing whenever the card stayed with it, which is every
/// window that published a digest with the operator's card switch off.
///
/// FAIL CLOSED ON ANYTHING ELSE. A payload this cannot read is a card nobody
/// can vouch for, and the process that composed it has already decided it is
/// not delivering it.
///
/// A LEG NAMED BY NOTHING REGISTERED IS SKIPPED. Both ends are one binary, so
/// the only way to reach it is a corrupted line; dropping that leg still
/// delivers the card to the destinations that do exist.
pub fn decode_handed_card(line: &str) -> Option<HandedCard> {
    if line.trim().is_empty() {
        return None;
    }
    let wire: serde_json::Value = serde_json::from_str(line).ok()?;
    let registered = pns_domain::registry::roster().names();
    Some(HandedCard {
        identity: SubmissionIdentity {
            producer: wire["producer"].as_str()?.to_string(),
            request_id: wire["request_id"].as_str()?.to_string(),
        },
        detail: wire["detail"].as_str()?.to_string(),
        legs: wire["legs"]
            .as_array()?
            .iter()
            .filter_map(|leg| {
                let named = leg["name"].as_str()?;
                Some(Leg {
                    name: registered.iter().copied().find(|name| *name == named)?,
                    mode: mode(leg["mode"].as_str()?)?,
                    decorative: leg["decorative"].as_bool()?,
                })
            })
            .collect(),
    })
}

fn mode(word: &str) -> Option<ReportMode> {
    [ReportMode::Silent, ReportMode::ReportOutcome]
        .into_iter()
        .find(|mode| mode.as_str() == word)
}

#[cfg(test)]
#[path = "recap_card_wire/tests.rs"]
mod tests;
