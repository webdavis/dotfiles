//! The daemon's core: what a scheduled job is, whether it fires, what a repeat
//! re-arms to, and the spool directory the two sides talk through.
//!
//! THE IPC IS A DIRECTORY, and that is the whole design. A short-lived process
//! registers work by writing ONE file; the daemon reads the directory on its
//! tick. There is no connection, no handshake, no reply and NOTHING FOR A HOOK
//! TO WAIT ON, which is the property every other choice here falls out of: a
//! daemon that is dead, wedged or mid-restart changes nothing about the write.
//!
//! The write is `main.rs`'s `publish_state_line` shape (a private 0600 temp
//! named by pid, then a rename) and the read is its `claim_by_rename` shape (a
//! rename decides the owner, because a plain unlink does NOT arbitrate on APFS:
//! measured, eight racing unlinkers were every one of them told they had
//! succeeded). Both are re-stated here rather than reused because both are
//! private to the composition root and these are library functions the hooks
//! call directly.

use std::io::{Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// One leased job: the whole of what the daemon knows how to do.
///
/// ONE PRIMITIVE, not two. The nag ("say something at T unless an answer
/// arrived") and the animation upkeep ("keep re-arming a short effect while a
/// loop is alive") reduce to the same record, so the daemon has one concept and
/// neither rider adds a second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    /// A stable name. It is the spool FILENAME, so re-registering the same id
    /// replaces the job by rename rather than stacking a second one.
    pub id: String,
    /// The earliest second this may run.
    pub due: u64,
    /// The LEASE: past this second the job is dropped, never run.
    pub until: u64,
    /// Seconds between repeats. Absent is a one-shot.
    pub every: Option<u64>,
    /// A marker name that cancels the job if it exists when the job would
    /// fire. The name is resolved inside the state directory by the caller, so
    /// this field is never a path.
    pub unless_marker: Option<String>,
    /// The argv THIS binary is re-executed with. Never another program.
    pub args: Vec<String>,
}

/// One job as one line: TAB-separated `key=value`, `args` a JSON array.
///
/// TABS RATHER THAN SPACES, which is what lets the argv keep its own spaces.
/// A detail string is free text, and JSON escaping turns a literal tab inside
/// it into `\t`, so no field value can carry the separator. The ids and marker
/// names that reach here are validated against a set with no control character
/// in it, so neither can either.
///
/// AN ABSENT FIELD IS NOT RENDERED, rather than rendered as a sentinel. A
/// sentinel would need a value no real marker could be named, and every
/// candidate for one is a legal marker name.
pub fn render(job: &Job) -> String {
    let mut fields = vec![
        format!("id={}", job.id),
        format!("due={}", job.due),
        format!("until={}", job.until),
    ];
    if let Some(every) = job.every {
        fields.push(format!("every={every}"));
    }
    if let Some(marker) = &job.unless_marker {
        fields.push(format!("marker={marker}"));
    }
    // LAST, and the only field whose value can be long: nothing about the
    // parse depends on the order, but a reader scanning a spool file sees the
    // short scalars first.
    fields.push(format!(
        "args={}",
        serde_json::to_string(&job.args).unwrap_or_else(|_| "[]".to_string())
    ));
    fields.join("\t")
}

/// The most a record may be. Generous against the caps validation applies to
/// the fields inside it (`ID_MAX`, `ARGS_BYTES_MAX`), so a record that trips
/// this one is not a job with a long detail: it is a file somebody else wrote.
pub const RECORD_MAX: usize = 8192;

/// One line back into a job, or the reason it is not one.
///
/// REFUSED, NEVER GUESSED AT, in `parse_config`'s style: a missing field, a
/// repeated one, an unknown one and a value of the wrong shape are each an
/// error NAMING the offender. A record half-read is a job whose remaining
/// fields somebody else's edit decided, and the daemon re-executes this binary
/// from it.
pub fn parse(line: &str) -> Result<Job, String> {
    if line.len() > RECORD_MAX {
        return Err(format!(
            "the record is {} bytes, past the {RECORD_MAX}-byte cap",
            line.len()
        ));
    }
    if line.is_empty() {
        return Err("the record is empty".to_string());
    }
    let mut id = None;
    let mut due = None;
    let mut until = None;
    let mut every = None;
    let mut marker = None;
    let mut args = None;
    for field in line.split('\t') {
        let (key, value) = field
            .split_once('=')
            .ok_or_else(|| format!("field `{field}` is not `key=value`"))?;
        // A REPEAT IS AN ERROR RATHER THAN A LAST-WINS, which is the whole
        // reason each slot is filled through this helper: taking the last of
        // two `due` fields is a guess about which one the writer meant.
        match key {
            "id" => fill(&mut id, key, value.to_string())?,
            "due" => fill(&mut due, key, count(key, value)?)?,
            "until" => fill(&mut until, key, count(key, value)?)?,
            "every" => fill(&mut every, key, count(key, value)?)?,
            "marker" => fill(&mut marker, key, value.to_string())?,
            "args" => fill(
                &mut args,
                key,
                serde_json::from_str::<Vec<String>>(value)
                    .map_err(|_| "field `args` is not a JSON list of words".to_string())?,
            )?,
            _ => return Err(format!("unknown field `{key}`")),
        }
    }
    Ok(Job {
        id: required(id, "id")?,
        due: required(due, "due")?,
        until: required(until, "until")?,
        every,
        unless_marker: marker,
        args: required(args, "args")?,
    })
}

/// The most an id or a marker name may be. Long enough for a session id with
/// a prefix on it, short enough that a spool filename stays a filename.
pub const ID_MAX: usize = 64;

/// True when a name may be a filename inside the state directory.
///
/// ITS OWN RULE rather than either of `safety`'s two, and the difference is
/// the point in both directions. `session_id_is_safe` refuses the colon, which
/// a job id needs (`nag:sess-123`); `pane_is_safe` admits `..` and a leading
/// dot, which a filename must not have. Sharing either would couple this rule
/// to a change made for a different reason.
fn name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= ID_MAX
        && !name.starts_with('.')
        && !name.contains("..")
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
}

/// The rules a job must satisfy WHEREVER it came from: the registration that
/// wrote it and the loop that read it back.
///
/// THE LOOP APPLIES IT TOO, which is the whole reason it is a function rather
/// than a check inside the registration. A hand-edited spool file must not be
/// able to do what a registration could not.
///
/// IT TAKES NO CLOCK, so it says the same thing at write time and at read
/// time. The bound that IS a function of now (`due` inside a window) lives in
/// `validate_registration`, because a job re-armed hours ago and read back on
/// a woken laptop is a lease decision, not a malformed record.
pub fn validate_shape(job: &Job) -> Result<(), String> {
    if !name_is_safe(&job.id) {
        return Err(format!(
            "`id` must be 1 to {ID_MAX} characters of letters, digits, `.`, `_`, `:` or `-`, \
             with no leading `.` and no `..`"
        ));
    }
    if let Some(marker) = &job.unless_marker
        && !name_is_safe(marker)
    {
        return Err(format!(
            "`marker` must be 1 to {ID_MAX} characters of letters, digits, `.`, `_`, `:` or `-`, \
             with no leading `.` and no `..`"
        ));
    }
    // BOUNDED ON BOTH SIDES. A repeat under the tick is a job the loop would
    // re-arm into the past on every pass, which is a spin; one past the
    // ceiling is a lease-length repeat nobody meant to write.
    if let Some(every) = job.every
        && !(MIN_EVERY_SECS..=EVERY_MAX_SECS).contains(&every)
    {
        return Err(format!(
            "`every` must be between {MIN_EVERY_SECS} and {EVERY_MAX_SECS} seconds"
        ));
    }
    if job.until < job.due {
        return Err("`until` is before `due`, so the lease ends before it starts".to_string());
    }
    if job.args.is_empty() {
        return Err("`args` is empty, so the job would re-execute pns with no event".to_string());
    }
    if job.args.len() > ARGS_MAX {
        return Err(format!("`args` has more than {ARGS_MAX} words"));
    }
    let bytes: usize = job.args.iter().map(String::len).sum();
    if bytes > ARGS_BYTES_MAX {
        return Err(format!("`args` is longer than {ARGS_BYTES_MAX} bytes"));
    }
    // THE RENDERED RECORD, NOT THE FIELDS THAT WENT INTO IT, which is the only
    // length the parser will ever see. `render` JSON-escapes the argv, so one
    // control character becomes six bytes and a run of them expands past this
    // cap while every field bound above is still satisfied. Checked here rather
    // than at the write, so a registration is refused BY NAME instead of being
    // accepted, written, and dropped by the daemon as unparseable on the next
    // tick.
    let rendered = render(job).len();
    if rendered > RECORD_MAX {
        return Err(format!(
            "the rendered record is {rendered} bytes, past the {RECORD_MAX}-byte cap"
        ));
    }
    Ok(())
}

/// The shape rules PLUS the one bound that needs a clock.
///
/// A `due` FAR FROM NOW IS REFUSED IN BOTH DIRECTIONS, per the two-sided-bound
/// rule: far in the future parks a job the lease can never expire, and far in
/// the past is a clock jump or a corrupt field rather than a schedule.
pub fn validate_registration(job: &Job, now: u64) -> Result<(), String> {
    validate_shape(job)?;
    if job.due.abs_diff(now) > DUE_WINDOW_SECS {
        return Err(format!(
            "`due` is more than {DUE_WINDOW_SECS} seconds from now"
        ));
    }
    Ok(())
}

/// What the loop does with one job on one tick.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Not yet. Left exactly where it was found.
    Wait,
    /// Run it.
    Fire,
    /// Never run it, and say which rule said so.
    Drop(Reason),
}

/// Why a job was dropped. TWO REASONS, NOT ONE STRING, because they send a
/// reader to two different places: a lease that ran out is a machine that was
/// down or a client that stopped refreshing, and a marker is the thing the job
/// was waiting to be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    LeaseExpired,
    MarkerPresent,
}

impl Reason {
    /// The half-sentence the log line carries.
    pub fn said(self) -> &'static str {
        match self {
            Reason::LeaseExpired => "its lease had expired",
            Reason::MarkerPresent => "its marker was already there",
        }
    }
}

/// Whether one job fires now, waits, or is dropped.
///
/// A TOTAL FUNCTION OF FOUR VALUES: the job, the second, whether the marker
/// exists, and whether a child THIS job already fired is still running. It
/// opens no file and reads no clock, which is what lets the window be swept a
/// second at a time in a unit test.
///
/// BOTH EDGES CLOSED. `due <= now <= until` fires, so a job whose lease is
/// exactly its due second still runs; one second past `until` never does.
///
/// THE LEASE IS CHECKED FIRST, then the marker, then whether a child is still
/// running, and the due second last: an expired job is dropped as expired
/// even when its marker also arrived, and a job whose answer came in is
/// dropped without ever being described as waiting.
///
/// A RUNNING CHILD ANSWERS `Wait`, NEVER `Drop`, so the occurrence stays due
/// and fires the tick after that child is gone rather than being lost. THE
/// SEAMLESS BREATH IS WHY THIS EXISTS: its last fade is issued to still be
/// running when the child exits, so the schedule alone can no longer promise
/// the previous child is gone before a second one starts. `rearm`'s
/// `now + every` still governs how soon the next occurrence is due, so a job
/// held here by a slow child does not burst once that child finally exits.
pub fn decide(job: &Job, now: u64, marker_exists: bool, running: bool) -> Verdict {
    if now > job.until {
        return Verdict::Drop(Reason::LeaseExpired);
    }
    if marker_exists {
        return Verdict::Drop(Reason::MarkerPresent);
    }
    if running {
        return Verdict::Wait;
    }
    if now < job.due {
        return Verdict::Wait;
    }
    Verdict::Fire
}

/// What a fired job leaves behind: the same job due again, or nothing.
///
/// `now + every`, NEVER `due + every`. A loop that reaches a job late (a busy
/// tick, a woken laptop) and re-armed from the OLD due would compute a next
/// due that is still in the past, fire again immediately, and keep firing
/// until it caught up: a burst of cards for a schedule that meant one.
///
/// `until` IS CARRIED OVER UNCHANGED, which is the property the lease exists
/// for. A repeat that renewed its own lease would run until the machine
/// stopped, with nobody refreshing it and nothing to notice that the client
/// which asked for it is gone.
///
/// AND THE LEASE IS WHAT ENDS THE REPEAT: a next occurrence past `until` can
/// never fire, so the job leaves NOTHING behind rather than a record whose own
/// due sits outside its own lease. That record would be a job the loop refuses
/// as malformed on its next pass, which is a true statement about a file this
/// code wrote and a confusing one to find in a log.
pub fn rearm(job: &Job, now: u64) -> Option<Job> {
    let due = now.saturating_add(job.every?);
    (due <= job.until).then(|| Job { due, ..job.clone() })
}

/// What the daemon says about itself each tick: which process it is, and when
/// it last looked.
///
/// AN AGE RATHER THAN A PID PROBE is what the doctor grades. A pid can be
/// reused, so `kill(pid, 0)` answers "some process exists" and not "this
/// daemon is alive"; the age of a file the daemon rewrites every second
/// answers the question that was actually asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Heartbeat {
    pub pid: u32,
    pub at: u64,
}

/// The heartbeat file's one line: the pid, a space, the epoch.
pub fn render_heartbeat(beat: &Heartbeat) -> String {
    format!("{} {}", beat.pid, beat.at)
}

/// That line read back, or None for anything this will not vouch for.
///
/// NO HEARTBEAT IS ITS OWN ANSWER, never a beat at epoch zero: a file some
/// other hand rewrote is not a beat, and reading one as zero would report a
/// running daemon as long dead or a dead one as running, depending on which
/// half was garbled. Pid zero is refused for the same reason `pid_is_gone`
/// refuses a non-positive one: it is not a process this tool started.
pub fn parse_heartbeat(line: &str) -> Option<Heartbeat> {
    let (pid, at) = line.trim().split_once(' ')?;
    let pid = u32::try_from(crate::parse_count(pid)?).ok()?;
    (pid > 0).then_some(Heartbeat {
        pid,
        at: crate::parse_count(at)?,
    })
}

/// How old a beat may be and still mean the daemon is running.
///
/// TEN TICKS, which is generous against a loop whose whole body is one
/// `read_dir` of a small directory. It is a small MULTIPLE of the tick rather
/// than the tick itself, so a machine under load that missed a beat is not
/// reported dead.
pub const HEARTBEAT_STALE_SECS: u64 = 10 * DEFAULT_TICK_SECS;

/// The tick, in whole seconds, for the one reader that grades against it.
const DEFAULT_TICK_SECS: u64 = 1;

/// The shortest repeat, which is the tick: a job cannot come round faster than
/// the loop looks.
const MIN_EVERY_SECS: u64 = 1;

/// The longest repeat. A day, which is far past the two riders' minutes and
/// short enough that a mistyped value is refused rather than scheduled.
pub const EVERY_MAX_SECS: u64 = 86_400;

/// How many argv words one job may carry. `pns`'s own event flags number under
/// a dozen; anything past this is not an event.
pub const ARGS_MAX: usize = 32;

/// How long the whole argv may be. The detail text is the only long field, and
/// the render already caps a card's preview far below this.
pub const ARGS_BYTES_MAX: usize = 4096;

/// How far from now a `due` may sit, in either direction. Thirty days.
pub const DUE_WINDOW_SECS: u64 = 30 * 24 * 60 * 60;

/// One slot, filled once. A second value for the same key is refused by name.
fn fill<T>(slot: &mut Option<T>, key: &str, value: T) -> Result<(), String> {
    if slot.is_some() {
        return Err(format!("field `{key}` appears more than once"));
    }
    *slot = Some(value);
    Ok(())
}

/// A required field, or the name of the one that is missing.
fn required<T>(slot: Option<T>, key: &str) -> Result<T, String> {
    slot.ok_or_else(|| format!("field `{key}` is missing"))
}

/// One numeric field, through the crate's own strict count.
///
/// `crate::parse_count` RATHER THAN `str::parse`, which is the same choice
/// every other reading in this crate makes: it refuses a leading `+`, a
/// leading zero, surrounding whitespace and anything past what the shell this
/// ports can hold, so a numeral nobody wrote as a plain count is unknown
/// rather than coerced.
fn count(key: &str, value: &str) -> Result<u64, String> {
    crate::parse_count(value).ok_or_else(|| format!("field `{key}` is not a plain count"))
}

#[cfg(test)]
mod tests {
    use super::{Job, Reason, Verdict, decide, parse, rearm, render, validate_shape};

    fn full() -> Job {
        Job {
            id: "nag:sess-123".to_string(),
            due: 1_700_000_000,
            until: 1_700_000_300,
            every: Some(30),
            unless_marker: Some("answered-sess-123".to_string()),
            args: vec![
                "--agent".to_string(),
                "pns".to_string(),
                "--detail".to_string(),
                "a nudge with spaces".to_string(),
            ],
        }
    }

    #[test]
    fn a_job_with_every_field_set_round_trips_through_its_on_disk_form() {
        let job = full();
        assert_eq!(parse(&render(&job)), Ok(job));
    }

    /// Every way a line can fail to be a record, each naming what was wrong.
    ///
    /// A GUESS IS THE FAILURE THIS PREVENTS. A record half-read is a job with
    /// a field somebody else's edit decided, and the daemon re-executes this
    /// binary from it.
    #[test]
    fn a_record_that_is_not_a_record_is_refused_by_name_rather_than_guessed_at() {
        let good = render(&full());
        for (case, line, named) in [
            ("empty", String::new(), "empty"),
            (
                "truncated mid-field",
                "id=x\tdue=100\tuntil=200\targ".to_string(),
                "arg",
            ),
            ("a field repeated", format!("{good}\tdue=5"), "due"),
            ("an unknown field", format!("{good}\tbogus=1"), "bogus"),
            (
                "a non-numeric due",
                good.replace("due=1700000000", "due=soon"),
                "due",
            ),
            (
                "a due past u64",
                good.replace("due=1700000000", "due=18446744073709551616"),
                "due",
            ),
            (
                "a negative every",
                good.replace("every=30", "every=-5"),
                "every",
            ),
            (
                "args that are not a list of words",
                good.replace("args=", "args=nonsense args="),
                "args",
            ),
            (
                "a missing required field",
                good.replace("until=1700000300\t", ""),
                "until",
            ),
            (
                "a record past the cap",
                format!("{good}\tmarker={}", "m".repeat(super::RECORD_MAX)),
                "cap",
            ),
        ] {
            let refusal = parse(&line).expect_err(&format!("{case} must be refused"));
            assert!(
                refusal.contains(named),
                "{case}: the refusal must name `{named}`, said: {refusal}"
            );
        }
    }

    /// THE ONE THAT STOPS A FILENAME BECOMING A PATH. The id is the spool
    /// entry's name and the marker is a name joined to the marker directory, so
    /// a separator or a parent reference in either writes or reads outside the
    /// state directory, silently.
    #[test]
    fn an_id_cannot_escape_the_spool_directory() {
        let over_long = "x".repeat(super::ID_MAX + 1);
        for (case, id) in [
            ("a parent reference", ".."),
            ("a parent reference inside a name", "a..b"),
            ("a path separator", "a/b"),
            ("a traversal", "../../etc/passwd"),
            ("a leading dot", ".hidden"),
            ("empty", ""),
            ("a control character", "a\u{7}b"),
            ("a newline", "a\nb"),
            ("a space", "a b"),
            ("over-long", over_long.as_str()),
        ] {
            let job = Job {
                id: id.to_string(),
                ..full()
            };
            let refusal = validate_shape(&job).expect_err(&format!("{case} must be refused"));
            assert!(
                refusal.contains("id"),
                "{case}: the refusal must name `id`, said: {refusal}"
            );
        }
        // The ordinary shape a rider will really register.
        assert_eq!(validate_shape(&full()), Ok(()));
        // The marker is a filename by the same road, so it is judged by the
        // same rule and refused under its own name.
        let job = Job {
            unless_marker: Some("../escape".to_string()),
            ..full()
        };
        let refusal = validate_shape(&job).expect_err("a marker that is a path must be refused");
        assert!(
            refusal.contains("marker"),
            "the refusal must name `marker`, said: {refusal}"
        );
    }

    /// The rest of the registration's refusals, each naming its own field.
    ///
    /// ADDED BEYOND THE BRIEF'S FIFTEEN because the id rule was the only one
    /// of the validation set that had a behavior of its own, and an unbounded
    /// `every`, a lease that ends before it starts and an unbounded argv are
    /// each a way for one registration to cost the daemon or the operator
    /// something without ever being refused.
    #[test]
    fn every_other_out_of_range_field_is_refused_by_name_too() {
        let long_word = "x".repeat(super::ARGS_BYTES_MAX);
        for (case, job, named) in [
            (
                "a repeat faster than the tick",
                Job {
                    every: Some(0),
                    ..full()
                },
                "every",
            ),
            (
                "a repeat past the ceiling",
                Job {
                    every: Some(super::EVERY_MAX_SECS + 1),
                    ..full()
                },
                "every",
            ),
            (
                "a lease that ends before it starts",
                Job {
                    due: 1_700_000_300,
                    until: 1_700_000_299,
                    ..full()
                },
                "until",
            ),
            (
                "no argv at all",
                Job {
                    args: Vec::new(),
                    ..full()
                },
                "args",
            ),
            (
                "more argv words than the cap",
                Job {
                    args: vec!["--local-only".to_string(); super::ARGS_MAX + 1],
                    ..full()
                },
                "args",
            ),
            (
                "an argv past the byte cap",
                Job {
                    args: vec!["--detail".to_string(), long_word.clone()],
                    ..full()
                },
                "args",
            ),
        ] {
            let refusal = validate_shape(&job).expect_err(&format!("{case} must be refused"));
            assert!(
                refusal.contains(named),
                "{case}: the refusal must name `{named}`, said: {refusal}"
            );
        }
        // Both edges of the lease are legal: a one-shot whose lease is exactly
        // its due second is the shape the nag registers.
        assert_eq!(
            validate_shape(&Job {
                due: 1_700_000_300,
                until: 1_700_000_300,
                every: None,
                ..full()
            }),
            Ok(())
        );
    }

    /// The one bound that is a function of the clock, so it lives apart from
    /// the shape rules the loop re-applies.
    #[test]
    fn a_due_outside_a_bounded_window_of_now_is_refused_at_registration() {
        let now = 1_700_000_000;
        for (case, due) in [
            ("far in the future", now + super::DUE_WINDOW_SECS + 1),
            ("far in the past", now - super::DUE_WINDOW_SECS - 1),
        ] {
            let job = Job {
                due,
                until: due + 60,
                ..full()
            };
            let refusal = super::validate_registration(&job, now)
                .expect_err(&format!("{case} must be refused"));
            assert!(
                refusal.contains("due"),
                "{case}: the refusal must name `due`, said: {refusal}"
            );
        }
        assert_eq!(super::validate_registration(&full(), now), Ok(()));
    }

    /// BOTH EDGES ARE CLOSED, and both are asserted, because a one-sided
    /// bound is the bug class this window is most likely to acquire.
    #[test]
    fn a_job_fires_only_inside_its_window_and_both_edges_are_closed() {
        let job = full();
        for (case, now, expected) in [
            ("a second before due", job.due - 1, Verdict::Wait),
            ("exactly at due", job.due, Verdict::Fire),
            ("inside the window", job.due + 1, Verdict::Fire),
            ("exactly at until", job.until, Verdict::Fire),
            (
                "a second past until",
                job.until + 1,
                Verdict::Drop(Reason::LeaseExpired),
            ),
        ] {
            assert_eq!(decide(&job, now, false, false), expected, "case: {case}");
        }
    }

    /// THE LATE-STORM RULE. A laptop that slept through a job wakes to a lease
    /// that expired while it was down, and the job is dropped rather than run
    /// late, because "the machine was asleep" and "the nudge is now pointless"
    /// are the same condition.
    #[test]
    fn a_job_whose_lease_expired_while_the_machine_slept_is_dropped_never_run_late() {
        let now = 1_700_003_600;
        let job = Job {
            due: now - 3_600,
            until: now - 3_540,
            ..full()
        };
        assert_eq!(
            decide(&job, now, false, false),
            Verdict::Drop(Reason::LeaseExpired)
        );
    }

    /// The nag primitive: an answer that arrived cancels the nudge before
    /// anything runs.
    #[test]
    fn a_present_marker_cancels_the_job_before_anything_runs() {
        let job = full();
        // Squarely inside the window, so nothing but the marker can be what
        // dropped it.
        assert_eq!(
            decide(&job, job.due + 1, true, false),
            Verdict::Drop(Reason::MarkerPresent)
        );
        assert_eq!(decide(&job, job.due + 1, false, false), Verdict::Fire);
    }

    /// THE SEAMLESS BREATH'S OWN GUARD. A schedule that ends with its last
    /// fade still in flight can no longer promise the previous child is gone
    /// by the time the next occurrence is due, so a live child answers `Wait`
    /// rather than `Fire`, exactly like a due second that has not arrived yet.
    #[test]
    fn a_running_child_holds_the_next_occurrence_to_a_wait_rather_than_a_fire() {
        let job = full();
        assert_eq!(
            decide(&job, job.due, false, true),
            Verdict::Wait,
            "due, with no marker, but its own child is still running"
        );
        assert_eq!(
            decide(&job, job.due, false, false),
            Verdict::Fire,
            "the control: the same job, the same second, with nothing running"
        );
    }

    /// A REPEAT CANNOT EXTEND ITS OWN LEASE, which is the assertion that
    /// matters here: a job that renewed `until` as well as `due` would run
    /// forever with nobody refreshing it, and the lamp it drives would lie in
    /// exactly the direction the lease exists to prevent.
    #[test]
    fn a_repeating_job_re_arms_at_now_plus_every_and_a_one_shot_does_not_re_arm() {
        let job = full();
        let now = job.due;
        let next = rearm(&job, now).expect("a repeating job re-arms");
        assert_eq!(next.due, now + 30);
        assert_eq!(next.until, job.until, "the lease is UNCHANGED");
        assert_eq!(next.id, job.id);
        assert_eq!(next.args, job.args);

        assert_eq!(
            rearm(
                &Job {
                    every: None,
                    ..full()
                },
                now
            ),
            None,
            "a one-shot leaves nothing behind"
        );

        // FROM NOW, NEVER FROM `due`. A job the loop reaches late (a busy
        // tick, a woken laptop) whose next due were `due + every` would still
        // be in the past, so the daemon would fire it again immediately and
        // keep firing until it caught up: one burst instead of one repeat.
        let late = now + 100;
        let caught_up = rearm(&job, late).expect("a repeating job re-arms");
        assert_eq!(caught_up.due, late + 30);
        assert_ne!(caught_up.due, job.due + 30);

        // AND THE LEASE IS WHAT ENDS A REPEAT. A next occurrence past `until`
        // can never fire, so the job leaves nothing behind rather than a
        // record whose own due sits outside its lease.
        let last = job.until - 1;
        assert_eq!(rearm(&job, last), None);
        assert_eq!(
            rearm(&job, job.until - 30).map(|next| next.due),
            Some(job.until),
            "a next occurrence landing exactly on the lease still re-arms"
        );
    }

    #[test]
    fn the_two_optional_fields_round_trip_as_absent_rather_than_as_a_sentinel() {
        let job = Job {
            every: None,
            unless_marker: None,
            ..full()
        };
        let rendered = render(&job);
        assert!(
            !rendered.contains("every=") && !rendered.contains("marker="),
            "an absent field is not rendered at all: {rendered}"
        );
        assert_eq!(parse(&rendered), Ok(job));
    }
}

// --- the spool directory ----------------------------------------------------

/// Where jobs are spooled, one file per job named by its id.
pub fn spool_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon")
}

/// Where the markers that cancel jobs live. A job carries a marker NAME, never
/// a path, and it is resolved here, so the field cannot become a general
/// filesystem probe.
pub fn marker_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-markers")
}

/// Where the daemon says it is alive.
///
/// BESIDE THE SPOOL AND NOT INSIDE IT: a heartbeat file in the spool directory
/// would be read as a job every tick, refused as unparseable and dropped, so
/// the daemon would spend its life deleting its own pulse.
pub fn heartbeat_path(state_dir: &Path) -> PathBuf {
    state_dir.join("daemon-heartbeat")
}

/// The mode every file this module writes carries, matching every other state
/// file the crate publishes.
const STATE_FILE_MODE: u32 = 0o600;

/// The prefix this module's own working files carry, and which no valid id can
/// start with.
///
/// `~` IS OUTSIDE THE ID CHARSET, which is what makes this a rule rather than a
/// convention: a claim and a pending write both live in the spool directory,
/// and the scan has to be able to tell them from a job without parsing them.
const WORKING_PREFIX: &str = "~";

/// What a start found where the spool should be.
#[derive(Debug, PartialEq, Eq)]
pub enum Startup {
    /// The spool is a directory and the loop may run.
    Ready,
    /// It may not, and the line saying why.
    ///
    /// EVERY REFUSAL HERE IS PERMANENT, which is the whole reason this is a
    /// type rather than a bool: relaunching cannot turn a symlink into a
    /// directory or make an unwritable state directory writable, so the caller
    /// exits 0 and lets `KeepAlive { SuccessfulExit = false }` keep the job
    /// DOWN. Exiting non-zero would relaunch it every ten seconds forever,
    /// which is the atuin restart loop (~6000 attempts in production) arriving
    /// through the refusal door instead of the crash door. A transient failure
    /// would belong in a second variant and there is none today.
    Refused(String),
}

/// The spool directory, made if it is missing and REFUSED rather than repaired
/// if something else is standing there.
///
/// `create_dir_all` FOLLOWS A SYMLINK, so a link where the spool should be
/// would silently put every job somewhere this tool did not choose. Checked
/// with `symlink_metadata` first, following `append_ring_line`'s own refusal at
/// a state path.
pub fn prepare_spool(state_dir: &Path) -> Startup {
    let spool = spool_dir(state_dir);
    if let Ok(found) = std::fs::symlink_metadata(&spool)
        && !found.is_dir()
    {
        return Startup::Refused(format!(
            "{} is not a directory; refusing to start",
            spool.display()
        ));
    }
    if let Err(error) = std::fs::create_dir_all(&spool) {
        return Startup::Refused(format!("the spool directory could not be made ({error})"));
    }
    Startup::Ready
}

/// Register one job: validated, then written by rename.
///
/// THE ERROR IS RETURNED, NEVER PRINTED. Every caller states its own fail
/// direction, and the one this exists for (a hook registering a nudge) drops it
/// the way a log line is dropped: silently, locally, and without touching the
/// return value of the thing that called it.
pub fn schedule(state_dir: &Path, job: &Job, now: u64) -> Result<(), String> {
    validate_registration(job, now)?;
    publish_job(&spool_dir(state_dir), job)
        .map_err(|error| format!("the spool write failed: {error}"))
}

/// Forget one job by id. Answers whether there was one.
pub fn cancel(state_dir: &Path, id: &str) -> Result<bool, String> {
    if !name_is_safe(id) {
        return Err(format!("`{id}` is not a job id"));
    }
    match std::fs::remove_file(spool_dir(state_dir).join(id)) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("the spool entry could not be removed: {error}")),
    }
}

/// How many jobs are spooled. A COUNT AND NEVER THE CONTENTS, following the
/// missed journal's structural privacy rule: the doctor answers "is anything
/// scheduled" and nothing here becomes a reader of what.
/// REGULAR FILES ONLY, so the word "job" in the doctor's sentence is earned. A
/// FIFO or a directory in the spool is something the loop refuses to open and
/// will never run, and counting it would report a job that cannot exist.
pub fn job_count(state_dir: &Path) -> usize {
    spool_entries(&spool_dir(state_dir))
        .into_iter()
        .filter(|entry| matches!(std::fs::symlink_metadata(entry), Ok(found) if found.is_file()))
        .count()
}

/// Every spool entry that could be a job, sorted so a tick is deterministic.
///
/// THIS MODULE'S OWN WORKING FILES ARE SKIPPED by their prefix, and an id can
/// never carry it, so a claim in flight is never mistaken for a job.
pub fn spool_entries(spool: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(spool)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            !entry
                .file_name()
                .to_string_lossy()
                .starts_with(WORKING_PREFIX)
        })
        .map(|entry| entry.path())
        .collect();
    entries.sort();
    entries
}

/// What one look at a spool entry found.
#[derive(Debug, PartialEq, Eq)]
pub enum Peeked {
    /// A record this daemon will act on.
    Job(Box<Job>),
    /// Not a regular file. LEFT ALONE AND NEVER OPENED, following
    /// `append_ring_line`'s own refusal at a state path: a FIFO here would
    /// block the read forever and stall every later tick, and a symlink is a
    /// write somewhere this tool did not choose.
    Irregular,
    /// A regular file that is not a usable record. Dropped rather than guessed
    /// at, carrying the reason.
    Unusable(String),
}

/// One look at a spool entry, taken WITHOUT claiming it.
///
/// THE PEEK IS READ-ONLY, so a job that is merely waiting is left exactly where
/// it was found: nothing is renamed, nothing is rewritten, and a registration
/// arriving in the same second cannot be overwritten by a put-back of the
/// record this tick had already read. A read-only peek is enough to decide to
/// do NOTHING; every decision that acts is taken again on a claimed record.
///
/// `expect_id` IS THE ID THE SPOOL FILENAME PROMISED, and a record that says a
/// different one is refused rather than acted on. The id is what a repeat
/// republishes under and what a cancel removes, so a file `A` whose record says
/// `id=B` would let a job re-arm itself on top of an unrelated one. On the
/// claim path the same name is passed, because a claim is the same record under
/// a working name and its id must still be the one it was published as.
pub fn peek(entry: &Path, expect_id: &str) -> Peeked {
    if !matches!(std::fs::symlink_metadata(entry), Ok(found) if found.is_file()) {
        return Peeked::Irregular;
    }
    let mut text = String::new();
    let read = std::fs::File::open(entry).and_then(|file| {
        // CAPPED AT ONE BYTE PAST THE RECORD CAP, so a file over the cap still
        // arrives over it and the parse refuses it rather than reading a
        // truncated record as a whole one.
        Read::take(file, RECORD_MAX as u64 + 1).read_to_string(&mut text)
    });
    if let Err(error) = read {
        return Peeked::Unusable(format!("it could not be read ({})", error.kind()));
    }
    match parse(text.trim_end_matches('\n')) {
        Err(refusal) => Peeked::Unusable(refusal),
        // THE SAME RULES THE REGISTRATION APPLIED, so a hand-edited spool file
        // cannot do what a registration could not.
        Ok(job) if job.id != expect_id => Peeked::Unusable(format!(
            "its `id` is `{}`, which is not the `{expect_id}` it was spooled as",
            job.id
        )),
        Ok(job) => match validate_shape(&job) {
            Err(refusal) => Peeked::Unusable(refusal),
            Ok(()) => Peeked::Job(Box::new(job)),
        },
    }
}

/// Whether the marker that cancels this job is there.
///
/// `symlink_metadata`, so a dangling symlink still counts as present: the
/// question is whether something wrote the marker, not whether it resolves.
/// A job with no marker is never cancelled by one.
///
/// THE DIRECTORY IS CHECKED BEFORE ANY NAME INSIDE IT, and a symlink standing
/// where it should be is refused, matching the spool's own startup refusal. A
/// validated name cannot escape the state directory by itself, but a link at
/// the directory carries the whole lookup somewhere this tool did not choose,
/// which turns the field back into the general filesystem probe the name rule
/// exists to prevent.
///
/// A REFUSED DIRECTORY READS AS NO MARKER, so the job runs. That is the fail
/// direction the rest of this crate takes: a marker that cannot be trusted
/// cancels nothing, and the cost is one extra card rather than a cancellation
/// somebody else's symlink decided.
pub fn marker_exists(state_dir: &Path, job: &Job) -> bool {
    let Some(marker) = job.unless_marker.as_ref() else {
        return false;
    };
    if !name_is_safe(marker) {
        return false;
    }
    let directory = marker_dir(state_dir);
    if !matches!(std::fs::symlink_metadata(&directory), Ok(found) if found.is_dir()) {
        return false;
    }
    std::fs::symlink_metadata(directory.join(marker)).is_ok()
}

/// One spool entry taken by rename, or None when it is already gone.
///
/// THE RENAME IS THE OWNERSHIP TEST, and a plain unlink is not one: measured on
/// macOS 26.2 (APFS) and recorded in `take_claim`'s own doc comment, eight
/// processes unlinking one path were every one of them told they had succeeded,
/// while 40 rounds of eight racers renaming gave exactly one winner every time.
/// So two daemons cannot both run one occurrence: the claim is taken BEFORE the
/// record is read for anything the daemon acts on, and the loser reads nothing.
///
/// THE HELD NAME CARRIES A PER-RUN SEQUENCE as well as the pid, for the reason
/// `take_claim`'s does: one name per process couples every claim in a run to
/// the first one, and a claim the run could not finish then occupies the name.
pub fn claim(entry: &Path) -> Option<PathBuf> {
    use std::sync::atomic::{AtomicU32, Ordering};
    static CLAIM_SEQ: AtomicU32 = AtomicU32::new(0);
    let name = entry.file_name()?;
    let claim = entry.with_file_name(format!(
        "{WORKING_PREFIX}claim.{}.{}.{}",
        std::process::id(),
        CLAIM_SEQ.fetch_add(1, Ordering::Relaxed),
        name.to_string_lossy()
    ));
    // NEVER RENAMED OVER A CLAIM ALREADY THERE, because a rename OVERWRITES and
    // the name is this run's alone: anything sitting at it is a job this
    // process claimed and could not finish, and losing it silently is worse
    // than leaving it.
    if std::fs::symlink_metadata(&claim).is_ok() {
        return None;
    }
    std::fs::rename(entry, &claim).ok()?;
    Some(claim)
}

/// One job written into the spool by rename, replacing whatever the id named.
///
/// A CLIENT'S WRITE, and the overwrite is the point: re-registering an id is a
/// REFRESH rather than a second job, so newest-signal-wins is what a rename
/// gives for free.
///
/// PRIVATE, WHICH IS THE ENFORCEMENT. `schedule` is the only way in, and the
/// daemon's own side of the library has `hand_back` and nothing else, so the
/// loop CANNOT overwrite a client's registration even by mistake: the call that
/// would do it is not in scope where the loop is written.
fn publish_job(spool: &Path, job: &Job) -> std::io::Result<()> {
    publish(
        &spool.join(&job.id),
        &pending_for(spool, &job.id),
        &render(job),
    )
}

/// One record the DAEMON holds put back into the spool, answering `true` when
/// it went back under its id and `false` when a client had already written
/// there and its record was left alone.
///
/// THE DAEMON'S ONLY WRITE, AND IT NEVER OVERWRITES A CLIENT. A re-arm and a
/// put-back are both this daemon restating a record it read moments ago; a
/// client registering the same id in that window has published a NEWER signal,
/// and a rename would silently replace it with the older one, taking its due,
/// its lease and its argv with it. `hard_link` fails with `AlreadyExists`
/// instead, so the client's record stands and the daemon's stale copy is thrown
/// away. That is the invariant the whole id-is-the-filename refresh rule rests
/// on, and it is the one a peek-then-claim loop could not keep.
///
/// `hard_link` RATHER THAN `create_new`, so the file that lands is the one the
/// temp already carries: mode, bytes and all, published in one step the way the
/// rename publishes. There is no window in which a reader can see the name with
/// nothing behind it.
pub fn hand_back(spool: &Path, job: &Job) -> std::io::Result<bool> {
    publish_if_absent(
        &spool.join(&job.id),
        &pending_for(spool, &job.id),
        &render(job),
    )
}

/// The private name a pending write is staged under. One per process and per
/// id, and outside the id charset, so a stage in flight is never read as a job.
fn pending_for(spool: &Path, id: &str) -> PathBuf {
    spool.join(format!(
        "{WORKING_PREFIX}pending.{}.{id}",
        std::process::id()
    ))
}

/// The daemon's own pulse, published the same way.
pub fn publish_heartbeat(state_dir: &Path, beat: &Heartbeat) -> std::io::Result<()> {
    publish(
        &heartbeat_path(state_dir),
        &state_dir.join(format!(
            "{WORKING_PREFIX}pending.{}.daemon-heartbeat",
            std::process::id()
        )),
        &render_heartbeat(beat),
    )
}

/// One line published atomically at 0600: `publish_state_line`'s shape, stated
/// here because that one is private to the composition root.
///
/// PUBLISHED BY RENAME. A plain write truncates first, so a reader landing
/// between the truncate and the bytes sees an empty file, which every reader of
/// these files reads as no state at all. The pending path sits in the SAME
/// directory, because a rename across filesystems is not one, and it carries
/// this process's id so two runs publishing at once cannot share one.
fn publish(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
    stage(path, pending, line)?;
    if let Err(error) = std::fs::rename(pending, path) {
        // Nothing half-written is left for the next tick to trip over.
        let _ = std::fs::remove_file(pending);
        return Err(error);
    }
    Ok(())
}

/// The same line published only when the name is FREE, answering whether it
/// landed there.
///
/// A LINK RATHER THAN A RENAME, because a rename has no create-if-absent form
/// and `link(2)` is the one call that publishes a complete file and refuses an
/// occupied name in the same step. The temp is unlinked either way, so a name
/// somebody else won leaves nothing behind.
fn publish_if_absent(path: &Path, pending: &Path, line: &str) -> std::io::Result<bool> {
    stage(path, pending, line)?;
    let landed = match std::fs::hard_link(pending, path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    };
    let _ = std::fs::remove_file(pending);
    landed
}

/// The bytes written to their private name, ready to be published under the
/// real one.
fn stage(path: &Path, pending: &Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // THE PENDING FILE CARRIES THE MODE, because publishing it is a rename or a
    // link and neither one sets a mode.
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(STATE_FILE_MODE)
        .open(pending)?;
    // AND AGAIN AFTER THE OPEN, because `mode` above applies only when the open
    // CREATES the file, and a run interrupted before its publish leaves one for
    // the next run of that pid to reuse.
    file.set_permissions(std::fs::Permissions::from_mode(STATE_FILE_MODE))?;
    file.write_all(format!("{line}\n").as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod spool_tests {
    use super::{
        Job, Peeked, Startup, claim, hand_back, job_count, marker_dir, marker_exists, parse, peek,
        prepare_spool, publish_job, render, spool_dir, validate_registration,
    };
    use std::path::{Path, PathBuf};

    const NOW: u64 = 1_700_000_000;

    /// A private directory per test, removed on every exit path including a
    /// panic, in `Sandbox`'s own shape.
    struct Scratch {
        root: PathBuf,
    }

    impl Scratch {
        fn new(name: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("pns-daemon-{}-{name}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("a scratch directory");
            Scratch { root }
        }

        fn spool(&self) -> PathBuf {
            let spool = spool_dir(&self.root);
            std::fs::create_dir_all(&spool).expect("a spool");
            spool
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn job(id: &str, due: u64) -> Job {
        Job {
            id: id.to_string(),
            due,
            until: due + 300,
            every: Some(30),
            unless_marker: None,
            args: vec!["--agent".to_string(), "pns".to_string()],
        }
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).expect("the record")
    }

    /// A REFRESH PUBLISHED WHILE THE JOB IS CLAIMED SURVIVES THE DAEMON'S
    /// RE-ARM.
    ///
    /// The daemon computes a repeat's next occurrence from the record it took;
    /// a client registering the same id in that window has published a NEWER
    /// signal. An overwriting rename would replace the client's due, lease and
    /// argv with the daemon's older reading, which is the id-is-the-filename
    /// refresh guarantee failing in the one direction nobody can observe.
    #[test]
    fn a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm() {
        let scratch = Scratch::new("refresh-beats-rearm");
        let spool = scratch.spool();
        let refreshed = Job {
            args: vec!["--agent".to_string(), "refreshed".to_string()],
            ..job("upkeep", NOW + 5)
        };
        publish_job(&spool, &refreshed).expect("the client's refresh");

        let stale = job("upkeep", NOW);
        assert!(
            !hand_back(&spool, &stale).expect("the re-arm"),
            "the daemon's re-arm must lose to a record already at the id"
        );
        assert_eq!(
            parse(read(&spool.join("upkeep")).trim_end()),
            Ok(refreshed),
            "the client's refresh is what stayed"
        );

        // THE UNMUTATED CONTROL: with the id free, the same call lands.
        let free = job("other", NOW);
        assert!(
            hand_back(&spool, &free).expect("a free id"),
            "a free id must take the daemon's write"
        );
        assert_eq!(parse(read(&spool.join("other")).trim_end()), Ok(free));
    }

    /// A REGISTRATION LANDING WHILE THE OLD RECORD IS CLAIMED IS NOT DELETED BY
    /// THE CLAIM CLEANUP.
    ///
    /// The claim is a RENAME, so the daemon's cleanup removes the working name
    /// it holds and never the id. A cleanup that unlinked the id instead would
    /// throw away the registration that arrived while the old occurrence ran.
    #[test]
    fn a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup() {
        let scratch = Scratch::new("registration-survives-cleanup");
        let spool = scratch.spool();
        let old = job("nag", NOW);
        publish_job(&spool, &old).expect("the old record");

        let held = claim(&spool.join("nag")).expect("the claim");
        assert!(
            !spool.join("nag").exists(),
            "a claim takes the name with it"
        );

        // The client registers again while the daemon holds the old record.
        let fresh = Job {
            args: vec!["--agent".to_string(), "fresh".to_string()],
            ..job("nag", NOW + 60)
        };
        publish_job(&spool, &fresh).expect("the new registration");

        std::fs::remove_file(&held).expect("the cleanup");
        assert_eq!(
            parse(read(&spool.join("nag")).trim_end()),
            Ok(fresh),
            "the registration that arrived during the claim is what survived"
        );
    }

    /// AN ARGV THAT PASSES EVERY FIELD BOUND AND STILL RENDERS PAST THE RECORD
    /// CAP IS REFUSED AT REGISTRATION.
    ///
    /// The bound on `args` counts the bytes handed in; the record carries them
    /// JSON-ESCAPED, so one control character becomes six bytes. Accepted, this
    /// wrote a file the daemon could only ever drop as unparseable: a schedule
    /// that reported success and could never run.
    #[test]
    fn an_argv_that_renders_past_the_record_cap_is_refused_by_name() {
        let control_characters = Job {
            args: vec!["\u{1}".repeat(4096)],
            ..job("oversized", NOW)
        };
        let refusal = validate_registration(&control_characters, NOW)
            .expect_err("a record past the cap must be refused");
        assert!(
            refusal.contains("rendered record") && refusal.contains("8192"),
            "the refusal must name the cap it broke: {refusal}"
        );

        // THE UNMUTATED CONTROL: the same 4096 bytes with nothing to escape
        // render inside the cap and are accepted.
        let plain = Job {
            args: vec!["a".repeat(4096)],
            ..job("ordinary", NOW)
        };
        assert_eq!(validate_registration(&plain, NOW), Ok(()));
    }

    /// A RECORD WHOSE `id` IS NOT ITS FILENAME IS REFUSED RATHER THAN ACTED ON.
    ///
    /// The id is what a repeat republishes under and what a cancel removes, so
    /// a file `a-job` whose record says `id=other-job` could re-arm itself on
    /// top of an unrelated job's record and replace it.
    #[test]
    fn a_record_whose_id_is_not_its_filename_is_refused() {
        let scratch = Scratch::new("id-must-match-the-filename");
        let spool = scratch.spool();
        let lying = spool.join("a-job");
        std::fs::write(&lying, format!("{}\n", render(&job("other-job", NOW)))).expect("a record");

        let Peeked::Unusable(refusal) = peek(&lying, "a-job") else {
            panic!("a record naming another id must be refused");
        };
        assert!(
            refusal.contains("other-job") && refusal.contains("a-job"),
            "the refusal must name both ids: {refusal}"
        );

        // THE UNMUTATED CONTROL: the same record under its own name is a job.
        let honest = spool.join("other-job");
        std::fs::write(&honest, format!("{}\n", render(&job("other-job", NOW)))).expect("a record");
        assert!(matches!(peek(&honest, "other-job"), Peeked::Job(_)));
    }

    /// A SYMLINK STANDING WHERE THE MARKERS DIRECTORY SHOULD BE CANCELS
    /// NOTHING.
    ///
    /// A validated marker name cannot escape the state directory by itself, but
    /// a link AT the directory carries the whole lookup somewhere this tool did
    /// not choose, which is the general filesystem probe the name rule exists
    /// to prevent. Refused reads as no marker, so the job runs: one extra card
    /// rather than a cancellation somebody else's symlink decided.
    #[test]
    fn a_symlinked_markers_directory_cancels_nothing() {
        let scratch = Scratch::new("markers-dir-must-be-real");
        let elsewhere = scratch.root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("another directory");
        std::fs::write(elsewhere.join("answered"), "").expect("a marker over there");
        std::os::unix::fs::symlink(&elsewhere, marker_dir(&scratch.root)).expect("the symlink");

        let waiting = Job {
            unless_marker: Some("answered".to_string()),
            ..job("nag", NOW)
        };
        assert!(
            !marker_exists(&scratch.root, &waiting),
            "a marker reached through a symlinked directory must not cancel a job"
        );

        // THE UNMUTATED CONTROL: a real directory with the same marker in it
        // cancels the job, so the refusal above is the link and not the name.
        let honest = Scratch::new("markers-dir-real");
        let markers = marker_dir(&honest.root);
        std::fs::create_dir_all(&markers).expect("the markers directory");
        std::fs::write(markers.join("answered"), "").expect("the marker");
        assert!(marker_exists(&honest.root, &waiting));
    }

    /// THE DOCTOR COUNTS JOBS, SO IT COUNTS ONLY WHAT COULD BE ONE.
    ///
    /// The loop refuses to open an irregular entry and will never run it, so
    /// counting it reports a job that cannot exist, in the one sentence an
    /// operator reads to find out whether anything is scheduled.
    #[test]
    fn the_job_count_counts_records_and_not_whatever_is_in_the_directory() {
        let scratch = Scratch::new("job-count-is-jobs");
        let spool = scratch.spool();
        publish_job(&spool, &job("real", NOW)).expect("a real job");
        std::fs::create_dir_all(spool.join("a-directory")).expect("a directory in the spool");
        assert_eq!(
            job_count(&scratch.root),
            1,
            "only the record is a job; a directory is not one"
        );
    }

    /// A SPOOL PATH THAT IS NOT A DIRECTORY IS A PERMANENT REFUSAL.
    ///
    /// `create_dir_all` follows a symlink, so the check has to come first, and
    /// NOTHING A RETRY DOES CHANGES IT: that is why the caller exits 0 and lets
    /// launchd keep the job down instead of relaunching it every ten seconds
    /// forever.
    #[test]
    fn a_spool_path_that_is_not_a_directory_is_a_permanent_refusal() {
        let scratch = Scratch::new("spool-must-be-a-directory");
        std::fs::write(spool_dir(&scratch.root), "not a directory").expect("a file in the way");

        let Startup::Refused(refusal) = prepare_spool(&scratch.root) else {
            panic!("a file where the spool should be must refuse the start");
        };
        assert!(
            refusal.contains("is not a directory"),
            "the refusal must say what it found: {refusal}"
        );

        // THE UNMUTATED CONTROL: an absent spool is MADE rather than refused.
        let clean = Scratch::new("spool-is-made");
        assert_eq!(prepare_spool(&clean.root), Startup::Ready);
        assert!(spool_dir(&clean.root).is_dir(), "the spool is created");
    }
}
