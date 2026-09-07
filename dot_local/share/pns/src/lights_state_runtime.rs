use crate::*;

/// The news record, or nothing at all for a file this cannot vouch for.
///
/// FAIL TO DARK, which is `parse_news`' own direction reached through the one
/// place that knows where the file lives: an unreadable record arms no lamp
/// rather than arming one about news nobody can name.
pub(crate) fn read_news(state: &Path) -> pns::lights::News {
    news_at(&state.join(LIGHTS_NEWS))
}

/// The same reading, taken at whichever path holds the record: the published
/// one, or the claim a merge is holding it under.
fn news_at(path: &Path) -> pns::lights::News {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|line| pns::lights::parse_news(&line))
        .unwrap_or_default()
}

/// Record that a turn just finished or just died.
///
/// WRITTEN ON THE EVENT PATH WHATEVER THE DELIVERY DID, which is the whole point
/// of a record separate from the journal: a card that was suppressed, muted or
/// dropped is exactly the news the unread lamp exists to carry.
///
/// OWNED BY RENAME FOR THE MERGE, which is this crate's rule for a record two
/// runs can write at once. Two events landing together (an agent that finished
/// beside one that died) each read, change their own field and publish the whole
/// line, so a plain read-modify-write loses the other's field: the record then
/// says a turn finished when one had also died, which is the red the lamp never
/// shows. Taking the file first means the winner merges a record nobody else can
/// still be reading.
///
/// A MISS IS RETRIED ONCE, because absent and held look the same from here: a
/// rename answers `NotFound` for a machine that has never recorded any news and
/// for one whose holder is mid-merge, and a holder publishes within three
/// syscalls. So the wait is paid once in a machine's life for the first record,
/// and it is what closes the window for every record after it.
///
/// THE RESIDUAL, STATED: a run whose second attempt also misses merges into
/// whatever it can read at the path instead, which is the winner's record once
/// the winner has published and nothing while it is still merging. Its cost is
/// one lamp colour, which is what fail-quiet buys everywhere on this path.
///
/// FAIL-QUIET, in `record_missed`'s style: a record that did not land costs one
/// lamp its colour, and this process has no reader for a complaint.
pub(crate) fn record_news(state: &Path, behaviour: pns::config::Behaviour, now: Option<u64>) {
    let Some(now) = now else {
        return;
    };
    // A WAIT IS NOT NEWS AND TOUCHES NOTHING, decided BEFORE the record is
    // claimed rather than after it is read. `news_after` is the one place that
    // knows which behaviours count, so this asks it rather than keeping a second
    // list; claiming for one that does not count would rename the record away
    // and have to put it back, which is a window over a file this is trying to
    // make safe.
    if pns::lights::news_after(pns::lights::News::default(), behaviour, now).is_none() {
        return;
    }
    let path = state.join(LIGHTS_NEWS);
    let claim = path.with_extension(format!("claim.{}", std::process::id()));
    let claimed = claim_news(&path, &claim);
    let held = news_at(if claimed { &claim } else { &path });
    if let Some(next) = pns::lights::news_after(held, behaviour, now) {
        // The failure is DROPPED here and nowhere else: see the doc comment.
        let _ = publish_state_line(&path, &pns::lights::render_news(&next));
    }
    if claimed {
        // THE CLAIM GOES WHETHER OR NOT THE PUBLISH LANDED, because the publish
        // above writes the whole record: a claim left behind would be a second
        // file holding a stale copy that nothing ever reads and nothing removes.
        let _ = std::fs::remove_file(&claim);
    }
}

/// Take the record for a merge, or answer that this run is merging blind.
fn claim_news(path: &Path, claim: &Path) -> bool {
    for attempt in 0..NEWS_CLAIM_ATTEMPTS {
        if std::fs::rename(path, claim).is_ok() {
            return true;
        }
        if attempt + 1 < NEWS_CLAIM_ATTEMPTS {
            std::thread::sleep(NEWS_CLAIM_WAIT);
        }
    }
    false
}

/// How many times a merge looks for the record before going ahead without it,
/// and how long it waits between two looks.
///
/// TWO LOOKS AND TWO MILLISECONDS, which is the whole recovery: a holder is
/// three syscalls from publishing, and the only other reason the file is not
/// there is a machine that has never recorded any news, which pays this wait
/// exactly once.
const NEWS_CLAIM_ATTEMPTS: u32 = 2;
const NEWS_CLAIM_WAIT: Duration = Duration::from_millis(2);
/// The held record's entries, path and phase both, or None for a record this
/// cannot read.
///
/// ABSENT AND UNREADABLE ARE DIFFERENT ANSWERS, and collapsing them into an
/// empty list is what made a corrupt record read as a house holding nothing.
/// The event path's pulse gate then flashed straight over a lamp that was
/// breathing, and no reader was told. The ordinary case, a machine holding
/// nothing at all, is an ABSENT file and still answers with an empty list.
///
/// THE ONE PARSE, shared by every reader: `held_lamps` is this with the phase
/// dropped, so the three path-only consumers (the event path's pulse gate, the
/// operator's return, and the mute) read bare paths off the very same tokens
/// the breath's resume reads a phase from, and neither can drift from the
/// other's idea of what a token means.
pub(crate) fn read_held(state: &Path) -> Option<Vec<pns::lights::HeldEntry>> {
    match std::fs::read_to_string(state.join(LIGHTS_HELD)) {
        Ok(line) => Some(
            line.split_whitespace()
                .map(pns::lights::parse_held_token)
                .collect(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(Vec::new()),
        Err(_) => None,
    }
}

/// The fixture paths a held write is currently holding, bare, or None for a
/// record this cannot read. See `read_held` for the phase this drops.
pub(crate) fn held_lamps(state: &Path) -> Option<Vec<String>> {
    read_held(state).map(|entries| entries.into_iter().map(|entry| entry.path).collect())
}

/// Record what is held now, or forget the file when nothing is.
///
/// ONE LINE, SPACE SEPARATED, because a fixture path is `light/<id>` or
/// `grouped_light/<id>` and neither can carry a space, and neither can carry
/// `@` or `:` either, which is what lets a phased token
/// (`light/<id>@<end-unix-ms>:<h|l>:<state>`) share the line with a bare one.
/// That keeps this a `publish_state_line` write like every other state file
/// rather than a second file format.
///
/// A TICK CAN REPUBLISH A GLOW THE RETURN JUST CLEARED, and that is a stated
/// limit rather than a rule. The tick reads its condition before it reaches the
/// bridge, so a present event that advances the return edge and clears the held
/// paths while an older tick is still resolving fixtures loses the race here:
/// that tick writes the glow and records it again. Nothing arbitrates, because
/// there is no lock between two processes that are deliberately independent.
/// The next present event clears it with no daemon at all, and the next tick
/// after it reads the advanced edge and finds no condition, so the exposure is
/// one refresh interval. It is unbounded only for a tick that was its lease's
/// LAST run, and there the lamp waits for the operator's return, which is the
/// event that clears it.
/// THE FAILURE IS RETURNED, not dropped, because the caller has to stop: a
/// lamp armed after a record that did not land is a lamp nothing in the system
/// knows the name of, and the return from an absence, the next tick and the
/// operator's own mute all put lamps out BY NAME off this file.
pub(crate) fn remember_held(state: &Path, held: &[pns::lights::HeldEntry]) -> std::io::Result<()> {
    let marker = state.join(LIGHTS_HELD);
    if held.is_empty() {
        return match std::fs::remove_file(&marker) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        };
    }
    let line = held
        .iter()
        .map(pns::lights::render_held_token)
        .collect::<Vec<_>>()
        .join(" ");
    publish_state_line(&marker, &line)
}

/// Say a complaint ONCE, and say it again only when it changes.
///
/// THE MARKER IS A PARAMETER because two paths say things at different rates
/// about different sets: the tick folds every refusal of a pass into one line,
/// and the event path says only what it read off the ad-hoc quiet file. Sharing
/// one memory would have each of them forgetting the other's line and repeating
/// it, which is the chatter this whole mechanism exists to stop.
pub(crate) fn say_lights_once(state: &Path, complaints: &[String], marker: &str) {
    let marker = state.join(marker);
    let remembered = std::fs::read_to_string(&marker).unwrap_or_default();
    match pns::lights::say(complaints, remembered.trim_end_matches('\n')) {
        pns::lights::Say::Nothing => {}
        pns::lights::Say::Aloud(said) => {
            for complaint in complaints {
                eprintln!("{complaint}");
            }
            let _ = publish_state_line(&marker, &said);
        }
        pns::lights::Say::Forget => {
            let _ = std::fs::remove_file(&marker);
        }
    }
}
/// Where the fixture paths a steady glow is holding are recorded.
pub(crate) const LIGHTS_HELD: &str = "lights-held";
/// Where the two news epochs live: the second a turn last finished, and the
/// second one last died.
///
/// ONE LINE AND TWO NUMBERS, which is what keeps this a `publish_state_line`
/// write like every other state file rather than a second file format, and what
/// makes it inherently capped: a record that cannot grow cannot collapse at a
/// cap either.
const LIGHTS_NEWS: &str = "lights-news";

#[cfg(test)]
#[path = "lights_state_runtime/tests.rs"]
mod lights_state_runtime_tests;
