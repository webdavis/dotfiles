use pns_domain::lights::{
    held::Held,
    mute::{MAX_MUTED_PLACES, Muted},
    phase::{HeldEntry, Phase},
    streak::Streak,
    unread::News,
};

/// One streak as one line: the two seconds, space separated, in
/// `render_heartbeat`'s shape.
pub fn render_streak(streak: &Streak) -> String {
    format!("{} {}", streak.since, streak.last_seen)
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_heartbeat`'s style. A file some other
/// hand rewrote is not a streak, and reading a garbled half as zero would
/// report a loop as having worked since 1970, which passes every threshold
/// there is and leaves a lamp breathing over nothing.
pub fn parse_streak(line: &str) -> Option<Streak> {
    let (since, last_seen) = line.trim_end_matches('\n').split_once(' ')?;
    Some(Streak {
        since: pns_domain::count::parse_count(since)?,
        last_seen: pns_domain::count::parse_count(last_seen)?,
    })
}

/// The record as one line: the two epochs, space separated, with `0` for a kind
/// that has not happened. `render_heartbeat`'s shape, and `render_streak`'s.
pub fn render_news(news: &News) -> String {
    format!(
        "{} {}",
        news.done_at.unwrap_or_default(),
        news.failed_at.unwrap_or_default()
    )
}

/// That line read back, or None for anything this will not vouch for.
///
/// REFUSED, NEVER GUESSED AT, in `parse_streak`'s style, and the fail direction
/// is DARK: a file some other hand rewrote yields no news, so the lamp stays
/// out rather than breathing about something nobody can name.
pub fn parse_news(line: &str) -> Option<News> {
    let (done, failed) = line.trim_end_matches('\n').split_once(' ')?;
    let epoch = |count: u64| (count > 0).then_some(count);
    Some(News {
        done_at: epoch(pns_domain::count::parse_count(done)?),
        failed_at: epoch(pns_domain::count::parse_count(failed)?),
    })
}

/// One held-record token, rendered: the bare path, or the path with its phase,
/// `@<end-unix-ms>:<brightness>:<state>`.
///
/// `@` AND `:` NEITHER APPEAR IN A FIXTURE PATH (`light/<id>` or
/// `grouped_light/<id>`, the id a bridge-issued UUID), and neither appears in
/// a state word, so the token round trips through the same whitespace-separated
/// line the bare record always used, with nothing to escape.
pub fn render_held_token(entry: &HeldEntry) -> String {
    match entry.resume {
        Some(phase) => format!(
            "{}@{}:{}:{}",
            entry.path,
            phase.end_unix_ms,
            phase.landed_on,
            phase.held.word()
        ),
        None => entry.path.clone(),
    }
}

/// One held-record token, parsed.
///
/// A MALFORMED SUFFIX IS NO PHASE, NEVER UNREADABLE: the record as a whole
/// already has an unreadable answer (`None`, held_lamps' own `Err(_)` arm) for
/// a file nothing here could open at all, and a token that arrived from an
/// older build, a hand edit, or a write this tick's own guard cut short is a
/// path this run still knows how to put out. Losing the phase costs one fade
/// of resume; inventing an unreadable path would cost the lamp.
pub fn parse_held_token(token: &str) -> HeldEntry {
    let Some((path, suffix)) = token.split_once('@') else {
        return HeldEntry::bare(token);
    };
    let phase = (|| {
        let (end_ms, rest) = suffix.split_once(':')?;
        let (landed_on, word) = rest.split_once(':')?;
        Some(Phase {
            end_unix_ms: end_ms.parse().ok()?,
            landed_on: landed_on.parse().ok()?,
            held: Held::from_word(word)?,
        })
    })();
    match phase {
        Some(resume) => HeldEntry {
            path: path.to_string(),
            resume: Some(resume),
        },
        None => HeldEntry::bare(path),
    }
}

/// The entries the state file holds, or ONE complaint naming what is wrong
/// with it.
///
/// IT REPORTS RATHER THAN GUESSES, and the fail DIRECTION is the caller's,
/// which is why it is not stated here: the two callers take opposite ones and
/// both are deliberate. `ad_hoc_quiet`, the lamp path, turns any complaint into
/// `Muting::Everything`, because a house with every lamp loud is the 3am the
/// mute was armed to prevent. `pns lights quiet`, the command, prints the
/// complaint and rebuilds from an empty list, because an operator standing in
/// front of it is losing what the file held and gets to see that rather than a
/// silent repair.
///
/// A LINE IS `<epoch> <place>` AND NOTHING ELSE, with the only leniency the ONE
/// trailing newline the publish itself writes. Padding is not something this
/// ever wrote, so a file carrying it was edited by something else: a `trim()`
/// here is exactly the leniency that read `" 9223372036854775807\n"` as a live
/// mute one module over.
///
/// THE PLACE IS THE REST OF THE LINE VERBATIM, spaces and all, because a room
/// is called `3F - Master Bedroom` and splitting on whitespace would make that
/// four fields. What it may not be is empty, or padded at either end, since
/// neither would ever match the name a family claims in.
pub fn muted_entries(contents: &str) -> Result<Vec<Muted>, String> {
    let held = contents.strip_suffix('\n').unwrap_or(contents);
    let lines: Vec<&str> = held.split('\n').collect();
    if lines.len() > MAX_MUTED_PLACES {
        return Err(quiet_state_error(format!(
            "{} lines, more than the {MAX_MUTED_PLACES} places it keeps",
            lines.len()
        )));
    }
    lines.iter().map(|line| muted_entry(line)).collect()
}

/// One line of it, or the complaint that quotes the line back.
fn muted_entry(line: &str) -> Result<Muted, String> {
    let refused = || quiet_state_error(format!("{line:?}, which is not an expiry and a place"));
    let (stated, place) = line.split_once(' ').ok_or_else(refused)?;
    if place.is_empty() || place.trim() != place {
        return Err(refused());
    }
    Ok(Muted {
        expiry: pns_domain::count::parse_count(stated).ok_or_else(refused)?,
        place: place.to_string(),
    })
}

/// One wording for every way the file can be wrong, since the operator's move
/// is the same for all of them and a second sentence would only make two
/// problems look like one.
fn quiet_state_error(what: String) -> String {
    format!(
        "pns: state error (lights-quiet holds {what}); nothing is quiet, and \
         the next pns lights quiet write replaces the file"
    )
}

/// The file's body: one line per entry, in the order they are kept.
///
/// NO TRAILING NEWLINE, because `publish_state_line` writes one, and the parse
/// strips exactly that one. Two would read back as an empty last line, which
/// the parse refuses, so the round trip is what keeps this honest.
pub fn render_muted(entries: &[Muted]) -> String {
    entries
        .iter()
        .map(|entry| format!("{} {}", entry.expiry, entry.place))
        .collect::<Vec<String>>()
        .join("\n")
}
