use pns_domain::Delivery;

/// The recap posted, with the one fallback the locked spec names.
///
/// SYNCHRONOUS INSIDE THIS PROCESS, and REPORTING, which is the mode whose
/// whole purpose is that a failure is visible. Nobody is behind this, and a
/// silently dropped recap is the exact failure the feature exists to prevent.
///
/// THE FALLBACK IS A REAL MECHANISM. hermes answers 404 for a route it does not
/// know and 502 when the target rejects the delivery, and only a 2xx is
/// `delivered`, so a thread route the operator has not prepared refuses loudly.
/// The same body then goes to the default route with ONE line saying why it
/// landed there, which is the locked "falls back to a plain #pns message".
///
/// A VERDICT, NEVER A SENTENCE. The retry fires on `Failed` and `Unlaunched`
/// alone; `Silent` is an executable channel that RAN and has no second surface
/// to answer on, and reading it as a failure would post every recap twice on
/// every machine with a shell channel installed.
///
/// ACCEPTED LIMIT, AND IT IS THE SAME RULE'S OTHER SIDE: on a machine running
/// EXECUTABLE channels (`PNS_CHANNELS_DIR` set), `deliver` always answers
/// `Silent` for a channel that ran, whatever the gateway then said. So a 404
/// from an unprepared `pns-recap` route is invisible there and this fallback
/// never fires; the recap goes to the thread route and stays there. Closing it
/// would mean an executable channel reporting a per-destination outcome, which
/// is a change to the channel contract itself and not to a recap.
///
/// ONE FALLBACK AND NO LOOP. A default route that refuses too is a gateway
/// problem, and a recap is not worth a retry storm against one.
///
/// ACCEPTED LIMIT ON THE CHARACTER CEILING: the fallback line is appended to a
/// body `recap::fit` has already fitted, so the second post may exceed
/// `recap::MAX_CHARS` by that one line. Fitting it in would mean composing the
/// body twice, once per route, on a path taken only when the first route
/// refused. The ceiling has 100 characters of headroom under the gateway's own
/// split threshold and this line is 82 characters plus its newline, so the post
/// still lands as one message.
pub fn post_return_recap(
    body: &str,
    thread: bool,
    mut deliver: impl FnMut(&str, &str) -> Vec<Delivery>,
) -> i32 {
    if !thread {
        deliver(body, "");
        return 0;
    }
    if !refused(&deliver(body, RECAP_ROUTE)) {
        return 0;
    }
    deliver(&format!("{body}\n{THREAD_UNAVAILABLE}"), "");
    0
}

/// Whether a dispatch refused the recap, which is the only thing that earns
/// the fallback. See `post_recap`.
fn refused(outcomes: &[Delivery]) -> bool {
    outcomes
        .iter()
        .any(|delivered| matches!(delivered, Delivery::Failed(_) | Delivery::Unlaunched(_)))
}

/// The hermes route a threaded recap posts to. ONE CONST rather than a key: a
/// second machine wanting another name can have the key the day it exists, and
/// the operator prepares this route in hermes either way.
const RECAP_ROUTE: &str = "pns-recap";

/// The line the fallback adds, so a recap in the wrong place says why it is
/// there rather than looking like the design.
const THREAD_UNAVAILABLE: &str =
    "(the pns-recap route did not take this, so it landed on the default route instead)";

#[cfg(test)]
mod tests;
