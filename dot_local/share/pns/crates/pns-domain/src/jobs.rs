//! The daemon's job policy: what a job is, what the loop decides about one,
//! what a heartbeat says, and the bounds a registration is held to.
//!
//! POLICY ONLY. Every function here is a total function of its arguments, with
//! no clock, no filesystem and no printing: the loop reads the world and hands
//! the readings in.
//!
//! The TAB codec and the spool's transactions stay in the legacy package,
//! because this crate takes no `serde_json` and opens no file. `validate_shape`
//! stays with them: its last rule caps the RENDERED record, so it is a fact
//! about the serialized form rather than about the job.

/// The most an id or a marker name may be. Long enough for a session id with
/// a prefix on it, short enough that a spool filename stays a filename.
pub const ID_MAX: usize = 64;

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

/// The most a record may be. Generous against the caps validation applies to
/// the fields inside it (`ID_MAX`, `ARGS_BYTES_MAX`), so a record that trips
/// this one is not a job with a long detail: it is a file somebody else wrote.
pub const RECORD_MAX: usize = 8192;

/// True when a name may be a filename inside the state directory.
///
/// ITS OWN RULE rather than either of `safety`'s two, and the difference is
/// the point in both directions. `session_id_is_safe` refuses the colon, which
/// a job id needs (`nag:sess-123`); `pane_is_safe` admits `..` and a leading
/// dot, which a filename must not have. Sharing either would couple this rule
/// to a change made for a different reason.
pub fn name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= ID_MAX
        && !name.starts_with('.')
        && !name.contains("..")
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | ':' | '-')
        })
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
    let pid = u32::try_from(crate::count::parse_count(pid)?).ok()?;
    (pid > 0).then_some(Heartbeat {
        pid,
        at: crate::count::parse_count(at)?,
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
pub const MIN_EVERY_SECS: u64 = 1;

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

#[cfg(test)]
mod tests;
