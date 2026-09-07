use crate::*;

/// What Focus is doing to this machine right now, in one sentence.
///
/// THE UNREADABLE STATE IS WHAT EARNS THE LINE. If the store is ever gated
/// behind Full Disk Access, moves, or changes schema, this feature dies OPEN
/// and SILENT: pns simply stops respecting Focus, and nothing else anywhere
/// would ever say so.
///
/// THE ACCEPTED LIMIT, stated rather than designed around: the parser is
/// TOTAL, so bytes that are not JSON at all, and a schema change that leaves
/// the file valid JSON, both read as "no Focus" rather than as an error. Only
/// a failed READ of the file itself reaches the last two sentences, and a
/// store that had stopped being readable in any useful sense would still be
/// reported as quiet. Telling those apart needs a positive assertion about a
/// shape Apple promises nothing about.
///
/// FIVE SENTENCES, because ABSENT AND UNREADABLE ARE DIFFERENT THINGS TO SAY,
/// which is the rule `decision_section` and `missed_line` already follow one
/// screen up. A machine that has never asserted a Focus has no store, and
/// telling that operator their database could not be read sends them after a
/// Full Disk Access grant that was never the problem.
pub(crate) fn focus_line(home: &str, silence: &[String]) -> String {
    if silence.is_empty() {
        return "pns doctor: focus awareness is off (no [focus] table names a mode to silence)"
            .to_string();
    }
    match focus_now(home, silence) {
        Ok(reading) => {
            let state = if reading.silenced {
                "pns doctor: a macOS Focus you named is ON, so banners, cards and pulses \
                 are suppressed"
            } else {
                "pns doctor: no macOS Focus you named is active"
            };
            // A CATALOG NOBODY CAN READ RESOLVES NO NAMES, so a config written
            // the way the template shows it silences nothing while this line
            // otherwise reports perfect health. WHICH entries are names is not
            // decidable without the very file that failed, so the clause is
            // said whenever the catalog failed and the feature is on.
            match reading.catalog {
                None => state.to_string(),
                Some(kind) => format!(
                    "{state}; the mode catalog could not be read ({kind}), so no Focus NAME \
                     can match and only a raw modeIdentifier still would"
                ),
            }
        }
        // ABSENT IS ITS OWN STATE, and the one this machine is in until macOS
        // first writes the store. Anything else is a permission problem, a
        // path holding something that is not a file, or a store past the read
        // ceiling, which is a different thing to say.
        //
        // IT REPORTS WHAT WAS OBSERVED, "no database was found", rather than
        // asserting there is none. Whether a Full Disk Access refusal can
        // arrive as not-found rather than as a permission error is not
        // provable on a machine that holds the grant, so the sentence is
        // written to stay true either way.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            "pns doctor: no Focus database was found on this machine, so no Focus is being \
             respected"
                .to_string()
        }
        Err(error) => format!("{FOCUS_UNREADABLE} ({}).", error.kind()),
    }
}
/// A Focus store that is there and cannot be read. Said HERE rather than in
/// the module, for the reason `DECISIONS_UNREADABLE` is, and carrying the KIND
/// for the reason its two neighbours do: gated, oversized and not-a-file are
/// three different investigations.
const FOCUS_UNREADABLE: &str =
    "pns doctor: the Focus database could not be read, so Focus is being ignored";
