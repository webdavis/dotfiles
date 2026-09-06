//! Which places are muted, until when, and what the report says.

/// One place the operator muted by hand, and the second that mute ends.
///
/// ONE FILE, ONE LINE PER PLACE, rather than a file per place: a room name is
/// the operator's own text, spaces and all, and a file per place would make it
/// a filename. Nothing in this crate turns typed text into a path unless a
/// predicate already vouches for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Muted {
    pub expiry: u64,
    pub place: String,
}
/// How many places the ad-hoc quiet keeps at once.
///
/// MORE PLACES THAN A HOUSE HAS, and it is a guard on a file rather than a
/// policy: the command republishes the whole file every time and drops what has
/// expired, so reaching this at all means something else has been writing to
/// it. Refusing the file whole is what keeps an unbounded read off the event
/// path.
pub const MAX_MUTED_PLACES: usize = 32;
/// How long a BARE mute lasts: from now until the operator's quiet hours end.
///
/// THE SCHEDULE IS `[plugins.hue] quiet_hours` and there is no second one. A
/// mute typed at bedtime is about the operator's night, not about one room's
/// own dim window, and a room's window is a rendering rule that has nothing to
/// say about how long a by-hand silence should last.
///
/// NONE WHEN EITHER READING IS MISSING. No schedule is the refusal above; no
/// clock is a mute nothing could time, and the caller already refuses without
/// one.
///
/// NOW AT THE END MINUTE IS A WHOLE DAY, not nothing. The window ends at this
/// second, so the next end is tomorrow's; a mute of zero seconds is not a mute,
/// and the operator asked for one.
pub fn bare_mute_secs(ends_at: Option<u16>, minutes_now: Option<u16>) -> Option<u64> {
    let (ends_at, now) = (ends_at?, minutes_now?);
    const DAY: u64 = 24 * 60;
    let until = (u64::from(ends_at) + DAY - u64::from(now)) % DAY;
    Some(if until == 0 { DAY } else { until } * 60)
}
/// What the file holds after one typed command: this place at a new expiry, or
/// gone, and every other place kept as it was.
///
/// ONE FUNCTION FOR BOTH VERBS, because they differ by one value: `off` is an
/// expiry that is not there. Written as two, the drop and the replace would be
/// two spellings of the same rebuild and only one of them would learn about the
/// pruning below.
///
/// EXPIRED ENTRIES ARE DROPPED AS IT GOES PAST, and that is not tidiness: this
/// file has a line cap and a machine that mutes a different room every night
/// would otherwise reach it and have the whole file refused, which is a corrupt
/// state the command inflicted on itself.
///
/// A CLOCK NOBODY CAN READ KEEPS EVERY OTHER ENTRY. Dropping what cannot be
/// judged would let one broken clock reading erase mutes the operator set and
/// can still see, and the only command that reaches here without a clock is
/// `off`, which has one place to remove and no opinion about the rest.
///
/// AND IT REFUSES A MUTE PAST THE CAP RATHER THAN WRITING ONE. `muted_entries`
/// rejects a file past `MAX_MUTED_PLACES` WHOLE and mutes nothing, so a command
/// that published one more line would cancel every mute on the machine at the
/// next event with nothing said anywhere. A refusal beats a truncate, which
/// would silently drop a mute the operator typed. `off` never refuses: it can
/// only shrink the file, and so can re-muting a place already in it.
pub fn muted_after(
    entries: &[Muted],
    place: &str,
    expiry: Option<u64>,
    now: Option<u64>,
) -> Result<Vec<Muted>, String> {
    let mut kept: Vec<Muted> = entries
        .iter()
        .filter(|entry| {
            entry.place != place
                && now.is_none_or(|now| crate::quiet::is_muted(Some(entry.expiry), Some(now)))
        })
        .cloned()
        .collect();
    if let Some(expiry) = expiry {
        if kept.len() >= MAX_MUTED_PLACES {
            return Err(format!(
                "pns: lights quiet: {MAX_MUTED_PLACES} places are already quiet, \
                 which is every line lights-quiet keeps; the mute was not set, \
                 and `pns lights quiet <place> off` ends one"
            ));
        }
        kept.push(Muted {
            expiry,
            place: place.to_string(),
        });
    }
    Ok(kept)
}
/// The places an ad-hoc quiet covers at this second.
///
/// THE VERDICT IS `quiet::is_muted`'S, never re-derived here, which is that
/// module's own rule: one property read by two readers that each decide it is
/// how a report and a behaviour come to disagree about whether a mute is on.
/// Half open comes with it, so a mute ends on the second it names.
///
/// THIS HELPER FAILS OPEN BY CONTRACT: on no clock, `live` judges every entry
/// unmuted and this answers empty. That is true of this function alone. Its
/// only production caller is the root, `ad_hoc_quiet`, and the root asks
/// whether the clock answered BEFORE it asks this: on no clock it returns
/// `Muting::Everything` without ever reaching this line.
pub fn muted_places(entries: &[Muted], now: Option<u64>) -> Vec<String> {
    live(entries, now)
        .map(|entry| entry.place.clone())
        .collect()
}
/// Why every lamp is quiet on a run whose clock would not answer, the root's
/// own line: `ad_hoc_quiet` prints it as a complaint, and this prints it as
/// the report, so an operator reading either sees the same sentence.
pub const NO_CLOCK_FOR_THE_MUTE: &str = "pns lights: the clock cannot be read, so no \
mute can be judged live; every lamp is quiet until it can";
/// What `pns lights quiet` prints, which is the whole file in the operator's
/// own vocabulary.
///
/// THE REPORT IS THE SAME READING THE LAMPS TAKE, entry for entry, because a
/// report that decided liveness for itself is how a command and a lamp come to
/// disagree about whether a room is quiet. ON NO CLOCK, the same answer the
/// root gives: every place quiet, said once, never per entry and never
/// "nothing is quiet", which would tell the operator the opposite of what
/// every lamp is about to do.
pub fn muted_report(entries: &[Muted], now: Option<u64>) -> Vec<String> {
    if now.is_none() {
        return vec![NO_CLOCK_FOR_THE_MUTE.to_string()];
    }
    let lines: Vec<String> = live(entries, now)
        .map(|entry| {
            let minutes = crate::quiet::minutes_left(entry.expiry, now);
            let unit = if minutes == 1 { "minute" } else { "minutes" };
            format!(
                "pns lights: `{}` is quiet for another {minutes} {unit}",
                entry.place
            )
        })
        .collect();
    if lines.is_empty() {
        return vec!["pns lights: nothing is quiet".to_string()];
    }
    lines
}
/// The entries still muted at this second.
fn live(entries: &[Muted], now: Option<u64>) -> impl Iterator<Item = &Muted> {
    entries
        .iter()
        .filter(move |entry| crate::quiet::is_muted(Some(entry.expiry), now))
}
