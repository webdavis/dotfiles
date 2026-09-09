//! What a code MEANS here, in plain words, so the reader needs no HTTP
//! knowledge.
//!
//! EVERY MEANING NAMES ITS CONCRETE SUBJECT. A line never says "the route",
//! "the key" or "the payload" in the abstract, because a reader holding a phone
//! cannot resolve a pronoun against a system they are not looking at. The route
//! name, the config key and the address are all short, so precision costs
//! almost nothing against the character budget.
//!
//! Meanings are keyed by destination AND code together, which is why this is
//! two tables rather than one: a 401 from hermes and a 401 from moshi name
//! different secrets.

use super::Failure;
use crate::retry::DeliveryOutcome;

/// The destination id the hermes wording answers to.
pub const DESTINATION_HERMES: &str = "hermes";

/// The destination id the moshi wording answers to. The plugin table is named
/// `mobile` and the backend behind it is moshi.
pub const DESTINATION_MOBILE: &str = "mobile";

/// The config key holding the hermes signing key, quoted verbatim in a message.
///
/// NAMING A CONFIG KEY INSIDE AN ERROR IS A COMMITMENT: rename it and these
/// messages go stale silently. A test in `pns-adapters`, which can see both this
/// and the live config schema, asserts the two agree, so a rename breaks the
/// build rather than the message.
pub const HERMES_KEY: &str = "[plugins.hermes] key";

/// The config key holding the moshi token. Same commitment as [`HERMES_KEY`].
pub const MOBILE_TOKEN: &str = "[plugins.mobile] token";

/// The `status` field: the code paired with its registered name, because a
/// reader may know one and not the other.
pub(super) fn status(outcome: DeliveryOutcome) -> String {
    match outcome {
        DeliveryOutcome::Status(code) => match name(code) {
            Some(name) => format!("HTTP {code} ({name})"),
            // An unregistered code still says what it was. Inventing a name for
            // it would be worse than admitting there is none.
            None => format!("HTTP {code}"),
        },
        DeliveryOutcome::NoResponse => "no response".to_string(),
        DeliveryOutcome::NoStatus => "bad URL".to_string(),
    }
}

/// The registered name of a status this system can actually receive. Not a
/// complete IANA table: a code pns has never seen from a destination it posts
/// to earns no wording, and [`status`] prints the bare number for it.
fn name(code: u16) -> Option<&'static str> {
    Some(match code {
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        410 => "Gone",
        413 => "Payload Too Large",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => return None,
    })
}

/// The `meaning` field for this failure, in this destination's words.
pub(super) fn meaning(failure: &Failure) -> String {
    let route = &failure.route;
    let address = &failure.address;
    if failure.destination == DESTINATION_MOBILE {
        return moshi(failure.outcome, address);
    }
    hermes(failure.outcome, route, address)
}

fn hermes(outcome: DeliveryOutcome, route: &str, address: &str) -> String {
    let code = match outcome {
        DeliveryOutcome::NoResponse => return format!("nothing answered at {address}"),
        DeliveryOutcome::NoStatus => {
            return format!("the URL pns built for {route} is malformed, nothing was sent");
        }
        DeliveryOutcome::Status(code) => code,
    };
    match code {
        400 => format!("hermes could not parse the body pns posted to {route}"),
        401 => format!("the {HERMES_KEY} is wrong or missing"),
        403 => format!("the {HERMES_KEY} may not post to the {route} route"),
        404 => format!("the hermes gateway has no route named {route}"),
        405 => format!("the {route} route exists but refuses a POST"),
        410 => format!("the {route} route existed once and has been removed"),
        // The design's table wrote these two without a subject. Both are
        // permanent, so the reader has to go and do something about a specific
        // route, and "the body" alone does not say which one.
        413 => format!("hermes refused the body pns posted to {route} as too large"),
        422 => format!("hermes parsed the body pns posted to {route} but rejected its fields"),
        408 => "hermes did not finish reading the request before its own timeout".to_string(),
        429 => "hermes is rate limiting and refused this one for now".to_string(),
        500 => "hermes accepted the request then failed while handling it".to_string(),
        // DELIBERATELY SURPRISING, both of them. A loopback gateway normally
        // has no proxy in front of it, so naming one makes an odd situation
        // visible instead of hiding it behind "something went wrong".
        502 => format!("a proxy in front of hermes could not reach it at {address}"),
        504 => "a proxy in front of hermes gave up waiting for it".to_string(),
        503 => "hermes is running but refusing work, which usually means a restart".to_string(),
        // The classifier is total, so the wording has to be too. It says which
        // way pns will treat the code, since that is the part that changes what
        // the reader should do.
        _ if outcome.class().is_permanent() => {
            format!("hermes refused the post to {route} and will not accept a repeat")
        }
        _ => format!("hermes did not accept the post to {route} this time"),
    }
}

fn moshi(outcome: DeliveryOutcome, address: &str) -> String {
    let code = match outcome {
        DeliveryOutcome::NoResponse => return format!("nothing answered at {address}"),
        DeliveryOutcome::NoStatus => {
            return format!("the URL pns built from {address} is malformed, nothing was sent");
        }
        DeliveryOutcome::Status(code) => code,
    };
    match code {
        401 => format!("the {MOBILE_TOKEN} is wrong or expired"),
        404 => format!("moshi has no endpoint at {address}"),
        _ if outcome.class().is_permanent() => {
            format!("moshi refused the card and will not accept a repeat, at {address}")
        }
        _ => format!("moshi did not accept the card this time, at {address}"),
    }
}
