use pns_domain::doctor::{Pairing, PairingReport};

/// The pairing, read off `status --json`, and the server sentence, read off
/// plain `status`. Either is `None` when that call gave no answer.
pub fn pairing_report(json: Option<&str>, plain: Option<&str>) -> PairingReport {
    PairingReport {
        pairing: pairing_of(json),
        server: plain
            .filter(|plain| within_cap(plain))
            .and_then(server_said),
    }
}

/// Whether an answer is small enough to be looked at.
///
/// THE SPAWN'S CEILING IS NOT THIS CHECK'S CAP, and the room between them is
/// what this function lives in. The reader bounds bytes as well as time now,
/// but it bounds them at `PAIRING_READ_MAX`, which is deliberately wider than
/// `ANSWER_MAX`: an answer between the two ARRIVED and is refused HERE, with
/// "moshi-hook answered something this cannot read", while one past the
/// reader's own ceiling never arrives and is reported as no answer at all.
/// Without this check a moshi-hook that answered at length inside its time
/// window would hand the whole thing to serde on one leg and have every line
/// of it scanned on the other. It is checked BEFORE either, which is the only
/// point where it means anything, and it is checked HERE rather than in the
/// shared bounded spawn: every other caller of that spawn reads a different
/// tool, and one of them is a condenser whose whole job is to answer at length.
fn within_cap(answer: &str) -> bool {
    answer.len() <= ANSWER_MAX
}

/// How much of an answer this check will read. Thousands of times the captured
/// 0.3.3 answer, which is a couple of hundred bytes, and still far short of
/// anything worth parsing by accident.
///
/// THE READER IS ASKED FOR TWICE THIS, at the `run_bounded` calls in the
/// composition root (`PAIRING_READ_MAX`), which is what keeps an over-cap
/// answer DISTINGUISHABLE from one exactly at the cap: a reader that stopped at
/// the cap itself would hand `within_cap` a truncated answer that passes, and
/// the refusal there would never fire again. The doubling rather than a single
/// byte of headroom is deliberate and is argued at `PAIRING_READ_MAX` itself;
/// what matters here is that this constant does NOT set the reader's ceiling,
/// so moving it does not move that ceiling with it.
pub const ANSWER_MAX: usize = 1024 * 1024;

/// What moshi said about the server, taken off the ONE line that begins with
/// the label at column zero.
///
/// NOTHING HERE MATCHES ON THE SENTENCE. pns has no stable way to tell "Moshi
/// Pro attached" from "host does not belong to this user token", and a prefix
/// or substring rule over moshi's prose would fail in the dangerous direction
/// the day the wording changes. The operator reads moshi's own words instead.
fn server_said(plain: &str) -> Option<String> {
    plain
        .lines()
        .find_map(|line| line.strip_prefix(SERVER_LABEL))
        .map(|said| said.trim().to_string())
        .filter(|said| !said.is_empty())
}

/// moshi's own label for the one line carrying a server verdict. A LINE
/// PREFIX, never a substring: moshi indents its detail lines, and a substring
/// rule would quote whichever of them said the word first.
const SERVER_LABEL: &str = "server:";

/// The pairing `status --json` described, and NOTHING ELSE OFF THAT OBJECT.
/// Three keys are read; `hooks` in particular is deliberately not one of them.
fn pairing_of(json: Option<&str>) -> Pairing {
    // NO ANSWER IS ITS OWN STATE AND NOTHING GUESSES PAST IT. The bounded
    // spawn answers `None` for a binary that is absent, one that hung past its
    // deadline and one that exited non-zero, and nothing downstream may claim
    // to know which of the three it was.
    let Some(json) = json else {
        return Pairing::NoAnswer;
    };
    // AN ANSWER TOO BIG TO READ IS AN ANSWER THIS CANNOT READ, which is a
    // state this already has a line for. It is NOT no-answer: moshi-hook ran
    // and said something, and the honest report is that pns declined to read
    // it rather than that nothing arrived.
    if !within_cap(json) {
        return Pairing::Unreadable;
    }
    let Ok(answer) = serde_json::from_str::<serde_json::Value>(json) else {
        return Pairing::Unreadable;
    };
    match answer.get(PAIRED).and_then(serde_json::Value::as_bool) {
        Some(true) => Pairing::Paired {
            host_id: named(&answer, "hostId"),
            display_name: named(&answer, "displayName"),
        },
        Some(false) => Pairing::Unpaired,
        // A key that is absent, or holds something other than a bool, is an
        // answer this cannot read. It is NOT read as unpaired: guessing the
        // one state that earns an exit 1 out of a shape nobody recognized is
        // how a doctor starts failing healthy machines.
        None => Pairing::Unreadable,
    }
}

/// One string moshi named, or the honest admission that it named none. The
/// measured 0.3.3 answer always carries both alongside `paired: true`, so this
/// is the shape nobody has seen rather than a case to design around.
fn named(answer: &serde_json::Value, key: &str) -> String {
    answer
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(NOT_REPORTED)
        .to_string()
}

/// The one key that moves the exit code, spelled once.
const PAIRED: &str = "paired";

/// What stands in for an identifier moshi did not name.
const NOT_REPORTED: &str = "not reported";
