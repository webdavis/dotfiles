use crate::{env_deadline, moshi_hook_bin, run_bounded};
use std::{process::Command, time::Duration};

/// What moshi-hook says about this host's pairing, in TWO BOUNDED SPAWNS of
/// one subcommand.
///
/// The split is a correctness argument rather than a style one. `status
/// --json` is local-only, measured at 77ms with the base URL pointed at an
/// unroutable host, and it carries the pairing fact pns grades. Plain `status`
/// is the only shape carrying a server verdict and is the only thing the
/// doctor puts on the network for its own sake. One plain-only call would put
/// the local fact behind the network, so an outage would read as "pairing
/// could not be checked" on a machine that could have answered.
///
/// `probe` IS NEVER CALLED. Measured on 0.3.3, it answers `running: true` and
/// `gateway: true` against a HOME holding no pairing at all while its hostId
/// disappears, so its daemon-side provenance cannot be stated honestly.
///
/// A FORWARD RISK, named rather than coded around: every pairing state exits 0
/// today, so a future moshi that exited non-zero when unpaired would come back
/// as no answer and be reported as "could not check" while the approval path
/// is really dead. A future moshi that renamed or dropped the `server:` line
/// degrades the other way, silently and safely.
///
/// THE WORST CASE IS THE TWO DEADLINES ADDED, not the larger of them: the legs
/// run one after the other, so a moshi-hook wedged on both puts 5s + 8s on a
/// hand-typed command, measured at 13.07 seconds. Ten seconds is not the
/// bound and nobody should treat it as one.
pub fn read_pairing() -> pns_domain::doctor::PairingReport {
    let binary = moshi_hook_bin();
    let mut json = Command::new(&binary);
    json.args(["status", "--json"]);
    // WELL PAST THE CHECK'S OWN CAP on both legs, so an answer over that cap
    // still ARRIVES over it: read to the cap exactly and a truncated answer
    // would pass the refusal that exists to catch it.
    let json = run_bounded(json, None, moshi_json_deadline(), PAIRING_READ_MAX);
    let mut plain = Command::new(&binary);
    plain.arg("status");
    let plain = run_bounded(plain, None, moshi_status_deadline(), PAIRING_READ_MAX);
    crate::pairing_report(json.as_deref(), plain.as_deref())
}

/// How much of moshi's answer is read off the wire, which is NOT the same
/// number as how much of it the check will look at.
///
/// TWICE WHAT `doctor::pairing_report` READS, and the doubling is the whole
/// point of the constant. The reader refuses anything past its own ceiling
/// (`system::run_bounded`), and the check refuses anything past
/// `doctor::ANSWER_MAX`, and those two refusals say DIFFERENT things: over the
/// reader's ceiling nothing usable arrived at all, while over the check's cap
/// moshi-hook ran and said something pns declined to read. Read to the check's
/// cap exactly and the second sentence would be unreachable, so the room
/// between them is what keeps it a state an operator can actually be told
/// about. It is still a bound: a child streaming without end is stopped here.
///
/// ACCEPTED LIMIT, PAST THIS CEILING: a moshi-hook that answers with more than
/// two megabytes is reported as a daemon that DID NOT ANSWER, because that is
/// the only thing the reader can say about an answer it refused to read. A
/// wedged daemon streaming prose is then diagnosed as a dead one, which sends
/// the operator to `brew services restart` rather than to the output. The
/// trade is deliberate: the alternative is reading without a ceiling to be
/// able to describe what came back, and the ceiling is the point.
const PAIRING_READ_MAX: u64 = 2 * crate::ANSWER_MAX as u64;

/// How long `moshi-hook status --json` may take.
///
/// GENEROUS AGAINST A MEASURED 77ms, and pinned here rather than inherited
/// from the probe runner's shared window: this leg reaches no network today,
/// and "today" is exactly why the bound has to be this function's own to state
/// and a test's own to move.
fn moshi_json_deadline() -> Duration {
    env_deadline("PNS_MOSHI_JSON_DEADLINE_MS").unwrap_or(MOSHI_JSON_DEADLINE)
}

const MOSHI_JSON_DEADLINE: Duration = Duration::from_secs(5);

/// How long plain `moshi-hook status` may take.
///
/// IT MUST EXCEED MOSHI'S OWN internal timeout, measured at about 5.1 seconds
/// against an unroutable base URL. Killing it mid-wait would throw away the
/// very `unavailable (...)` sentence that explains the delay, which is the one
/// thing this call is for.
fn moshi_status_deadline() -> Duration {
    env_deadline("PNS_MOSHI_STATUS_DEADLINE_MS").unwrap_or(MOSHI_STATUS_DEADLINE)
}

const MOSHI_STATUS_DEADLINE: Duration = Duration::from_secs(8);
