//! The nag policy: what an unanswered approval IS, what its names are, when
//! one is too old to act on, what the fire decides about one, and what the
//! nudge card reads.
//!
//! POLICY ONLY. Every function here is a total function of its arguments, with
//! no config, no clock, no environment and no printing.
//!
//! The JSON codec and the three path builders stay in the legacy package: this
//! crate takes no `serde_json`, and the paths are grammar the composition root
//! resolves against a state directory it owns.

/// One approval waiting on the operator: everything the nudge card is built
/// from, plus the second it started waiting.
///
/// THE CARD'S OWN FIELDS AND NOTHING ELSE. The session id is NOT among them:
/// it is the record's FILENAME, so carrying it inside as well would be two
/// copies of one fact that a hand edit could set against each other.
///
/// `armed` IS THE PROMPT'S OWN SECOND, read once by the hook that wrote the
/// record. The fire is a different process minutes later and its clock read is
/// the OTHER end of the measurement; taking both from the fire would make
/// every record look freshly armed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Record {
    pub agent: String,
    pub project: String,
    pub branch: String,
    /// The permission prompt's own text, which is the operator's free text and
    /// the reason this file is JSON.
    pub detail: String,
    pub pane: String,
    pub armed: u64,
}

/// The answered marker's NAME, never a path: the daemon resolves it inside its
/// own marker directory, which is what keeps the field from becoming a general
/// filesystem probe.
pub fn marker_name(session_id: &str) -> Option<String> {
    usable(session_id).map(|id| format!("{MARKER_PREFIX}{id}"))
}

/// The daemon job's id for one session. ONE JOB PER APPROVAL, and the id is the
/// spool filename, so a second approval in one session REPLACES the job rather
/// than stacking a second one.
pub fn job_id(session_id: &str) -> Option<String> {
    usable(session_id).map(|id| format!("{JOB_PREFIX}{id}"))
}

/// The session id a record's file name belongs to, or None for a name that is
/// not a record.
///
/// THE SUFFIX IS THE WHOLE TEST, which is what keeps a claim out of the fire's
/// enumeration: a claim is `<name>.claim.<pid>` and ends in digits, so it can
/// never be read back as a record and taken a second time.
pub fn session_of(file_name: &str) -> Option<String> {
    usable(file_name.strip_suffix(RECORD_SUFFIX)?).map(str::to_string)
}

/// What a record's file name ends in.
pub const RECORD_SUFFIX: &str = ".pending";

/// What an answered marker's name starts with.
const MARKER_PREFIX: &str = "nag-";

/// What a nudge job's id starts with. A COLON, which `session_id_is_safe`
/// refuses and the daemon's own id rule admits, so a job id can never be
/// mistaken for a session id.
const JOB_PREFIX: &str = "nag:";

/// How long a lock on disk is believed before it is read as the leavings of a
/// crash.
///
/// A MINUTE IS A WIDE MARGIN OVER THE WORK IT COVERS. The holder claims every
/// record by rename before it delivers anything, so a fire that broke in later
/// would find an empty directory in any case; the lock only has to cover the
/// enumeration, which is one `read_dir` and a rename per entry. What the wait
/// costs when the holder really did crash is one nudge window, which is the
/// safe direction.
pub const FIRE_STALE_SECS: u64 = 60;

/// The longest session id these names can carry.
///
/// THE DAEMON'S OWN CAP, LESS THE LONGEST PREFIX, and it is a correctness bound
/// rather than tidiness: past it `job_id` is refused at registration, so a
/// longer id would write a record no nudge could ever be scheduled for and the
/// file would sit there unread. Refused here, where refusing costs one arm that
/// never happens.
pub const MAX_SESSION_ID_CHARS: usize = crate::jobs::ID_MAX - JOB_PREFIX.len();

/// One session id that may become every name above, or None.
///
/// `session_id_is_safe` PLUS TWO BOUNDS THIS LAYER OWNS. That predicate answers
/// "may this be a filename"; a LEADING DOT passes it and is refused here for
/// the daemon's reason (a hidden file is not a name this writes), and the
/// length is the daemon's registration cap arriving one layer early.
pub fn usable(session_id: &str) -> Option<&str> {
    (crate::safety::session_id_is_safe(session_id)
        && !session_id.starts_with('.')
        && session_id.len() <= MAX_SESSION_ID_CHARS)
        .then_some(session_id)
}

// --- what the card says -----------------------------------------------------

/// The nudge card's detail: how long, and for exactly one approval, what was
/// asked.
///
/// TWO SHAPES, AND THE SECOND NAMES NO QUESTION. Coalescing is the operator's
/// own ruling, and a coalesced card that quoted one of the questions would
/// imply it was THE one; the card is capped at a couple of hundred characters
/// on the phone anyway, so naming one and hiding the rest is the worst of both.
///
/// BOTH ARE STATEMENTS AND NEITHER IS A QUESTION. A nudge goes through
/// `run_event` rather than the blocked path, so it structurally cannot carry
/// Allow and Deny, and the wording must not suggest it does: moshi's own card
/// is still the one that can be answered.
///
/// AN EMPTY QUESTION ENDS THE SENTENCE rather than trailing a separator over
/// nothing, which is what a record written before its detail arrived would
/// otherwise read as.
pub fn nudge(waiting: usize, oldest_secs: u64, question: &str) -> String {
    if waiting != 1 {
        return format!(
            "{waiting} approvals waiting, oldest {}",
            waited(oldest_secs)
        );
    }
    let waited = waited(oldest_secs);
    match question.is_empty() {
        true => format!("still waiting {waited}"),
        false => format!("still waiting {waited}: {question}"),
    }
}

/// How long it has been, in the largest unit that still reads as a count.
///
/// `decision_log`'s own ladder without its " ago", which is a different
/// sentence: that one dates an entry in a report and this one measures a wait
/// inside one. Sharing it would mean one caller trimming a suffix off the
/// other's words.
///
/// TWO CALLERS, ONE RENDERER: the card's own sentence and the doctor's line
/// about the schedule. An operator who reads "carded again after 5m" and then
/// "still waiting 5m" is reading one unit in two places, and a second spelling
/// is how those two come to disagree.
pub fn waited(seconds: u64) -> String {
    match seconds {
        ..60 => format!("{seconds}s"),
        60..3_600 => format!("{}m", seconds / 60),
        _ => format!("{}h", seconds / 3_600),
    }
}

// --- how old is too old -----------------------------------------------------

/// Whether a record is past the moment it was worth acting on, BOUNDED ON BOTH
/// SIDES.
///
/// TWICE THE SCHEDULE is the cap: one `after_secs` is the wait the operator
/// asked for, and a second is the slack a busy tick or a woken laptop is
/// allowed. Past that the prompt is not news, it is history, and the card that
/// wakes a laptop to describe last night is the case this exists for.
///
/// AND A FUTURE `armed` IS STALE, which is bug class 2: a clock that moved
/// backwards, or a hand-edited epoch, would otherwise read as fresh forever and
/// nudge on every fire until somebody deleted the file.
///
/// SATURATING, though `after_secs` is already range-bound at parse time: the
/// bound is the config layer's and this arithmetic is not entitled to assume
/// it, because a record's `armed` comes off disk.
///
/// ONE FUNCTION, TWO ENFORCERS, and they are not redundant. The daemon's lease
/// drops the JOB, so a machine that slept through the window never spawns at
/// all; this judges RECORDS, which is a different set, because a fire wakes on
/// one approval's timer and enumerates siblings whose own jobs have not fired
/// yet.
pub fn is_stale(armed: u64, now: u64, after_secs: u64) -> bool {
    armed > now || now > armed.saturating_add(after_secs.saturating_mul(2))
}

// --- what the fire decides about one record ---------------------------------

/// What one claimed record is worth on this fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fate {
    /// It is waiting, and this card is about it.
    Count,
    /// It is not, and the reason a reader would want.
    Drop(Dropped),
}

/// Why a claimed record was dropped. THREE REASONS RATHER THAN ONE STRING,
/// because they send a reader to three different places: a file nobody can read
/// is somebody else's write, an answered approval is the feature working, and a
/// stale one is a machine that was asleep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    Unreadable,
    Answered,
    Stale,
}

/// Whether one claimed record earns a place in this card.
///
/// A TOTAL FUNCTION OF ITS ARGUMENTS: it opens no file and reads no clock, so
/// the whole decision is swept in a table.
///
/// THE ORDER IS THE REPORT'S ORDER, not a shortcut. Unreadable first, because
/// nothing else can be asked of a record that did not parse. The marker before
/// the cap, so an approval the operator ANSWERED is reported as answered rather
/// than as merely old, which is the difference between the feature working and
/// the machine having been asleep.
///
/// EVERY DROP MEANS SILENCE, which is the rule the whole design falls out of:
/// an unreadable, absent, ambiguous or failed input resolves to no nudge, never
/// to a nudge taken on a guess.
pub fn fate(record: Option<&Record>, marker_exists: bool, now: u64, after_secs: u64) -> Fate {
    let Some(record) = record else {
        return Fate::Drop(Dropped::Unreadable);
    };
    if marker_exists {
        return Fate::Drop(Dropped::Answered);
    }
    if is_stale(record.armed, now, after_secs) {
        return Fate::Drop(Dropped::Stale);
    }
    Fate::Count
}

#[cfg(test)]
mod tests;
