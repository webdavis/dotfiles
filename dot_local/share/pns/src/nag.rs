//! The nag: what an unanswered approval leaves behind, and what a nudge about
//! it says.
//!
//! POLICY ONLY, in `decision_log.rs`'s style: every function here is a total
//! function of its arguments, with no config, no clock, no environment and no
//! printing. The composition root reads the world, writes the files and
//! delivers the card; this module says what a record IS, what its files are
//! called, when one is too old to act on, what the fire decides about one, and
//! what the card reads.

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

/// One record as one JSON object, `missed_notifications::entry`'s shape.
///
/// JSON AND NOT `key=value`, for the journal's own reason: the detail is a
/// permission prompt's text and can carry a newline, a tab or a quote, and a
/// line-oriented form would let one of those forge a second record. BUILT WITH
/// `json!` AND NEVER WITH `format!`, which is this repo's "build JSON with
/// `jq -n --arg`" rule in Rust.
pub fn render(record: &Record) -> String {
    serde_json::json!({
        "agent": record.agent,
        "project": record.project,
        "branch": record.branch,
        "detail": record.detail,
        "pane": record.pane,
        "armed": record.armed,
    })
    .to_string()
}

/// That object read back, or None for text that is not one.
///
/// PARSED BY KEY, never by position, which is the journal's own rule: the
/// writer's key order belongs to `serde_json` and no reader should depend on
/// it.
///
/// A MISSING KEY READS AS EMPTY rather than refusing the record, again
/// following the journal: a short record degrades to a thinner card, and every
/// value here already has an empty reading. A missing `armed` is second zero,
/// which the staleness cap then refuses as far too old, so the degraded case
/// resolves to silence rather than to a nudge about an unknown moment.
///
/// TEXT THAT IS NOT A JSON OBJECT IS REFUSED, because there is nothing to
/// degrade to: a file somebody else wrote at this path is not a thinner record
/// of ours, and the fire drops the claim rather than guessing at it.
pub fn parse(text: &str) -> Option<Record> {
    let fields: serde_json::Map<String, serde_json::Value> = serde_json::from_str(text).ok()?;
    Some(Record {
        agent: string(&fields, "agent"),
        project: string(&fields, "project"),
        branch: string(&fields, "branch"),
        detail: string(&fields, "detail"),
        pane: string(&fields, "pane"),
        armed: fields
            .get("armed")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default(),
    })
}

/// One string field, or empty when the key is absent or holds something else.
fn string(fields: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    fields
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

// --- the names --------------------------------------------------------------

/// Where the records live: a SUBDIRECTORY of the state directory, deliberately.
///
/// The state directory is otherwise flat, but the fire ENUMERATES records, and
/// a flat directory would mean pattern-matching every other state file on every
/// wake. The daemon's own `daemon/` and `daemon-markers/` set the precedent.
pub fn nag_dir(state_dir: &std::path::Path) -> std::path::PathBuf {
    state_dir.join("nag")
}

/// The record's path for one session, or None for an id that may not be a
/// filename.
pub fn record_path(state_dir: &std::path::Path, session_id: &str) -> Option<std::path::PathBuf> {
    usable(session_id).map(|id| nag_dir(state_dir).join(format!("{id}{RECORD_SUFFIX}")))
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

/// The name this process claims one record under, BUILT FROM THE WHOLE FILE
/// NAME.
///
/// NEVER `Path::with_extension`, which replaces everything after the LAST dot.
/// A harness session id may contain dots, so a claim derived from anything
/// short of the full name can collapse two sessions onto one claim: one loses
/// its nudge and the other can be delivered twice. Appending to the whole name
/// cannot, whatever the id contains.
///
/// THE RENAME IS THE OWNERSHIP TEST and this is the name it renames to. A plain
/// unlink does not arbitrate on APFS (measured, eight racers all told they
/// succeeded), which is why the fire takes a record by rename before reading it
/// for anything.
pub fn claim_path(record: &std::path::Path, pid: u32) -> std::path::PathBuf {
    let name = record.file_name().unwrap_or_default().to_string_lossy();
    record.with_file_name(format!("{name}{CLAIM_INFIX}{pid}"))
}

/// What a record's file name ends in.
pub const RECORD_SUFFIX: &str = ".pending";

/// What an answered marker's name starts with.
const MARKER_PREFIX: &str = "nag-";

/// What a nudge job's id starts with. A COLON, which `session_id_is_safe`
/// refuses and the daemon's own id rule admits, so a job id can never be
/// mistaken for a session id.
const JOB_PREFIX: &str = "nag:";

/// What a held claim's name carries before the pid.
const CLAIM_INFIX: &str = ".claim.";

/// The whole FIRE's lock, one well-known name beside the records.
///
/// NOT A RECORD NAME, so it can never be enumerated as one: a record ends in
/// `RECORD_SUFFIX` and this does not, and neither does the claim taken from it.
pub const FIRE_LOCK: &str = "fire.lock";

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
pub const MAX_SESSION_ID_CHARS: usize = crate::daemon::ID_MAX - JOB_PREFIX.len();

/// One session id that may become every name above, or None.
///
/// `session_id_is_safe` PLUS TWO BOUNDS THIS LAYER OWNS. That predicate answers
/// "may this be a filename"; a LEADING DOT passes it and is refused here for
/// the daemon's reason (a hidden file is not a name this writes), and the
/// length is the daemon's registration cap arriving one layer early.
fn usable(session_id: &str) -> Option<&str> {
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
mod tests {
    use super::{
        Dropped, Fate, MAX_SESSION_ID_CHARS, Record, claim_path, fate, is_stale, job_id,
        marker_name, nudge, parse, record_path, render, session_of,
    };
    use std::path::Path;

    #[test]
    fn a_record_with_every_field_set_round_trips_through_its_on_disk_form() {
        let record = Record {
            agent: "claude".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            // The operator's own question, carrying every character a
            // line-oriented `key=value` form would forge a second record out
            // of.
            detail: "Bash: cargo test\t\"quoted\"\nsecond line".to_string(),
            pane: "wW:p21".to_string(),
            armed: 1_700_000_000,
        };
        assert_eq!(parse(&render(&record)), Some(record));
    }

    #[test]
    fn a_record_missing_a_key_degrades_to_a_thinner_one_and_a_line_that_is_not_json_is_refused() {
        // THE JOURNAL'S OWN RULE, applied here: every value has an empty
        // reading, so a short record still cards. A missing `armed` reads as
        // second zero, which the staleness cap refuses, so the degraded case
        // resolves to SILENCE rather than to a nudge about an unknown moment.
        assert_eq!(
            parse(r#"{"detail":"may I"}"#),
            Some(Record {
                detail: "may I".to_string(),
                ..Record::default()
            })
        );
        // And text that is not an object has nothing to degrade to.
        for refused in ["", "not json", "[1,2]", "\"a string\"", "{"] {
            assert_eq!(parse(refused), None, "{refused} is not a record");
        }
    }

    // --- the names ---------------------------------------------------------

    #[test]
    fn an_ordinary_session_id_names_a_record_a_marker_a_job_and_a_claim() {
        let state = Path::new("/s");
        assert_eq!(
            record_path(state, "abc-123"),
            Some(Path::new("/s/nag/abc-123.pending").to_path_buf())
        );
        assert_eq!(marker_name("abc-123"), Some("nag-abc-123".to_string()));
        assert_eq!(job_id("abc-123"), Some("nag:abc-123".to_string()));
        // AND BACK: the fire enumerates files and has only the name to work
        // from, so the record's own name is where the session id comes from.
        assert_eq!(session_of("abc-123.pending"), Some("abc-123".to_string()));
        assert_eq!(session_of("abc-123"), None, "a record ends in .pending");
        assert_eq!(
            session_of("abc-123.pending.claim.7"),
            None,
            "a claim is outside the enumeration the fire matches on"
        );
    }

    #[test]
    fn a_session_id_that_cannot_be_a_filename_names_nothing_at_all() {
        // FAIL IN THE SAFE DIRECTION, which here is arming nothing: an id that
        // cannot become a name is one no record, marker or job is written for.
        let state = Path::new("/s");
        for refused in [
            "",
            "..",
            "../etc/passwd",
            "a/b",
            ".hidden",
            "a\u{7}b",
            "a\nb",
            "a b",
            "a:b",
            &"x".repeat(MAX_SESSION_ID_CHARS + 1),
        ] {
            assert_eq!(
                record_path(state, refused),
                None,
                "{refused:?} is not a name"
            );
            assert_eq!(marker_name(refused), None, "{refused:?} is not a name");
            assert_eq!(job_id(refused), None, "{refused:?} is not a name");
        }
        // THE CEILING IS EXACTLY WHERE THE DAEMON STOPS. `nag:<id>` is the job
        // id and `nag-<id>` the marker, and both are refused past the daemon's
        // own cap: a longer id would write a record no registration could ever
        // schedule a nudge for, so it is refused where it is cheap.
        assert!(record_path(state, &"x".repeat(MAX_SESSION_ID_CHARS)).is_some());
    }

    #[test]
    fn two_ids_that_differ_only_after_a_dot_claim_two_different_names() {
        // THE ROW THAT MATTERS. A claim name taken from anything but the WHOLE
        // file name collapses `a.b` and `a.c` onto one claim: one session loses
        // its nudge and the other can be delivered twice. Dots are legal in a
        // harness session id (`session_id_is_safe` admits them), so this is a
        // real pair rather than a hypothetical one.
        let state = Path::new("/s");
        let first = claim_path(&record_path(state, "a.b").expect("a.b names a record"), 7);
        let second = claim_path(&record_path(state, "a.c").expect("a.c names a record"), 7);
        assert_ne!(first, second);
        assert_eq!(
            first,
            Path::new("/s/nag/a.b.pending.claim.7").to_path_buf(),
            "the whole file name, suffix and all, carries into the claim"
        );
    }

    // --- what the card says ------------------------------------------------

    #[test]
    fn one_approval_is_nudged_with_its_own_question_and_how_long_it_has_waited() {
        assert_eq!(
            nudge(1, 300, "Bash: cargo test"),
            "still waiting 5m: Bash: cargo test"
        );
        // THE UNIT A HUMAN WOULD SAY IT IN, which under a minute is seconds:
        // the floor is thirty, so a drill really does read in seconds.
        assert_eq!(
            nudge(1, 45, "Bash: cargo test"),
            "still waiting 45s: Bash: cargo test"
        );
        // A record whose detail never arrived says the waiting and stops: a
        // trailing separator with nothing after it reads as a truncated card.
        assert_eq!(nudge(1, 300, ""), "still waiting 5m");
    }

    #[test]
    fn several_approvals_are_one_card_naming_the_count_and_no_question_at_all() {
        // THE OPERATOR'S COALESCING RULING. Naming ONE question implies it is
        // THE one, and the card is capped on the phone anyway, so the multi
        // case names none.
        let said = nudge(3, 720, "Bash: cargo test");
        assert_eq!(said, "3 approvals waiting, oldest 12m");
        assert!(
            !said.contains("cargo test"),
            "no question text reaches a coalesced card"
        );
        // AND BOTH ARE STATEMENTS, NEVER QUESTIONS, which is what keeps the
        // card from reading as a second answerable prompt: a nudge goes through
        // `run_event` and structurally cannot carry Allow and Deny.
        assert!(!said.contains('?'));
        assert!(!nudge(1, 300, "Bash: cargo test").contains('?'));
    }

    // --- how old is too old ------------------------------------------------

    #[test]
    fn a_record_is_too_old_in_both_directions_and_never_in_only_one() {
        const AFTER: u64 = 300;
        const NOW: u64 = 1_700_000_000;
        for (case, armed, stale) in [
            ("armed a hundred seconds ago", NOW - 100, false),
            ("armed exactly at the cap", NOW - 2 * AFTER, false),
            ("armed one second past the cap", NOW - 2 * AFTER - 1, true),
            ("armed last night", NOW - 7_200, true),
            // BUG CLASS 2, and the half a one-sided implementation passes
            // without: a clock that moved backwards, or a hand-edited epoch,
            // must not read as fresh forever.
            ("armed one second in the future", NOW + 1, true),
            ("armed far in the future", NOW + 86_400, true),
        ] {
            assert_eq!(is_stale(armed, NOW, AFTER), stale, "{case}");
        }
    }

    // --- what the fire decides about one record ----------------------------

    #[test]
    fn a_record_is_counted_only_when_nothing_says_otherwise() {
        const AFTER: u64 = 300;
        const NOW: u64 = 1_700_000_000;
        let fresh = Record {
            armed: NOW - 100,
            ..Record::default()
        };
        let old = Record {
            armed: NOW - 7_200,
            ..Record::default()
        };
        for (case, record, marker, expected) in [
            ("nothing says otherwise", Some(&fresh), false, Fate::Count),
            (
                "the marker arrived while we were waking",
                Some(&fresh),
                true,
                Fate::Drop(Dropped::Answered),
            ),
            (
                "no marker, but the moment has passed",
                Some(&old),
                false,
                Fate::Drop(Dropped::Stale),
            ),
            // THE MARKER OUTRANKS THE CAP, so an approval that was answered is
            // reported as answered rather than as merely old.
            ("both", Some(&old), true, Fate::Drop(Dropped::Answered)),
            (
                "nothing readable was there at all",
                None,
                false,
                Fate::Drop(Dropped::Unreadable),
            ),
        ] {
            assert_eq!(fate(record, marker, NOW, AFTER), expected, "{case}");
        }
    }
}
