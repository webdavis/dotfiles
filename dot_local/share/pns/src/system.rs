//! The IO edge: the five probe traits implemented against the real machine.
//!
//! WHAT LIVES HERE AND WHAT DOES NOT. Everything here runs a command and hands
//! the bytes to a parser; every parser is a free function taking `&str`, so a
//! test drives fixture output and never spawns anything. The DECISIONS all live
//! in `surface`, `presence` and `routing`, which is why nothing in this module
//! compares, thresholds or judges: it says what the machine reported, and
//! `surface` says what that means.
//!
//! The runner seam exists for the same reason: a test substitutes the command
//! output, so the suite never reads the live machine. That matters more than
//! usual here, because these readings are of the developer's own desk and
//! phone, and a suite that took them would answer differently every run.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// Runs a command and returns its stdout, or `None` when it cannot be run or
/// exits non-zero. The seam every probe reads the world through.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;
}

/// The production runner: spawns the command under a deadline and keeps its
/// stdout.
///
/// EVERY PROBE IS BOUNDED. A wedged herdr, ioreg, pgrep or ps would otherwise
/// hold a notification open indefinitely, and the readings all have a
/// fail-direction already: no answer reads as unknown, which never suppresses.
pub struct SystemCommandRunner;

/// One window for every probe. All of them answer in milliseconds, so this is
/// generous and still far short of a hang.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

/// One ceiling for every probe's OUTPUT. A registry dump, a process list and a
/// herdr layout are kilobytes, so a mebibyte is generous by three orders of
/// magnitude and still a bound: a probe that answered in gigabytes is a probe
/// that is not answering, and the callers all read no answer as unknown.
pub const PROBE_READ_MAX: u64 = 1024 * 1024;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let mut command = Command::new(program);
        command.args(args);
        run_bounded(command, None, PROBE_DEADLINE, PROBE_READ_MAX)
    }
}

/// Run a command with a deadline, returning its stdout on success.
///
/// There is no wait-with-timeout in the standard library and macOS ships no
/// `timeout(1)`, so the wait happens on a thread and the child is killed when
/// the window closes. Every spawn on a notification path is bounded: the
/// notification is worth less than the turn it reports on.
///
/// BOUNDED IN BYTES AS WELL AS IN TIME, which it was not. The read was a
/// `read_to_end` into a growing `Vec`, so "bounded" meant a child could hand
/// back as much as it managed to write inside the window, and the caller only
/// found out how much AFTER it was all in memory. That was academic while every
/// caller was a probe answering in kilobytes and stopped being academic the
/// moment one of them became an operator-named command running a model for
/// minutes. `max_bytes` is the reader's own ceiling: past it the pipe is closed
/// under the child, which is also what stops it writing.
///
/// AND PAST THE CEILING IS NO ANSWER, which is the DEADLINE'S OWN DIRECTION
/// rather than a second one. A truncated answer is the dangerous shape here:
/// a process list cut at the ceiling has lost its last rows and a JSON listing
/// has stopped mid-object, and both arrive at a caller looking exactly like a
/// complete short answer, so the caller acts on a reading that is missing the
/// part that mattered. Every caller reads no answer as unknown, and unknown
/// never suppresses. The reader is asked for one byte PAST the ceiling, which
/// is what keeps "over the cap" and "exactly at the cap" two different
/// answers, so the bound stays inclusive like every other bound in this crate.
pub fn run_bounded(
    mut command: Command,
    stdin_text: Option<&str>,
    deadline: Duration,
    max_bytes: u64,
) -> Option<String> {
    let expires_at = std::time::Instant::now() + deadline;
    let mut child = command
        .stdin(if stdin_text.is_some() {
            std::process::Stdio::piped()
        } else {
            std::process::Stdio::null()
        })
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;

    // The WRITE is inside the window too: a child that never reads its stdin
    // blocks the writer, and doing it before the clock started meant the
    // deadline never covered the case.
    let stdin_text = stdin_text.map(String::from);
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take()?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let (Some(text), Some(mut pipe)) = (stdin_text, stdin.take()) {
            let _ = std::io::Write::write_all(&mut pipe, text.as_bytes());
        }
        // Dropping stdin closes it, which is what tells the child to stop
        // reading; without it a child waiting on EOF never exits.
        drop(stdin);
        // THE READ IS CAPPED: `take` stops at the ceiling instead of growing
        // the buffer to whatever the child felt like writing, and the pipe
        // closing under it is what stops the child writing more. One byte past
        // the ceiling is asked for so the reader downstream can tell a child
        // that went over from one that stopped exactly on it.
        let mut output = Vec::new();
        let mut capped = std::io::Read::take(&mut stdout, max_bytes.saturating_add(1));
        let _ = std::io::Read::read_to_end(&mut capped, &mut output);
        // The BYTES travel, not a string: the size that matters is the size on
        // the wire, and a lossy conversion grows an invalid byte into three.
        let _ = sender.send(output);
    });

    // Over the ceiling is the same no-answer a blown deadline is; see above.
    let output = receiver
        .recv_timeout(deadline)
        .ok()
        .filter(|bytes: &Vec<u8>| bytes.len() as u64 <= max_bytes);
    // Closed stdout is not an exited process: a child can close it and sleep,
    // so the wait is polled against the SAME deadline rather than blocking.
    let status = match output.is_some() {
        true => wait_until(&mut child, expires_at, std::thread::sleep),
        false => None,
    };
    let Some(status) = status else {
        let _ = child.kill();
        let _ = child.wait();
        return None;
    };
    // A command that failed has no reading to give: every caller here treats
    // no answer as unknown, which is the honest report.
    //
    // LOSSY, and only now that the size has been judged: every reading here is
    // read line by line downstream, so one invalid byte must cost its own line
    // rather than the whole answer, and `read_to_string` would refuse the lot.
    status
        .success()
        .then_some(output)
        .flatten()
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

/// Poll a child to exit, up to a deadline. There is no wait-with-timeout in
/// the standard library and macOS ships no `timeout(1)`.
///
/// THE SLEEPER IS A PARAMETER because the schedule is only worth anything at
/// the line that sleeps it. `next_poll_interval` can compute the whole backoff
/// correctly while this loop sleeps a flat ceiling, which is the shape the
/// latency fix removed, so a test watches the durations this hands its sleeper
/// rather than a clock.
fn wait_until(
    child: &mut std::process::Child,
    expires_at: std::time::Instant,
    mut sleep: impl FnMut(Duration),
) -> Option<std::process::ExitStatus> {
    let mut interval = FIRST_POLL_INTERVAL;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Err(_) => return None,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= expires_at {
            return None;
        }
        sleep(interval);
        interval = next_poll_interval(interval);
    }
}

/// The LONGEST a bounded wait sleeps between checks. Long enough not to spin a
/// core while a genuinely wedged child runs out its deadline.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// And the SHORTEST, which is where every wait starts.
///
/// THE CEILING USED TO BE THE ONLY INTERVAL, and it was charged to every
/// bounded spawn. This wait begins after the child's stdout has already hit
/// EOF, which is a child on its way out, so the check that matters is the one
/// taken microseconds later and the ceiling is what a run pays for missing it.
const FIRST_POLL_INTERVAL: Duration = Duration::from_micros(200);

/// The next gap between checks: doubled, and never past the ceiling. The
/// backoff is what keeps the fast start from becoming thousands of wakeups a
/// second on the one path where a child really is wedged.
fn next_poll_interval(current: Duration) -> Duration {
    current.saturating_mul(2).min(POLL_INTERVAL)
}

/// The idle counter's own units, as the registry reports them.
const IOREG_IDLE_KEY: &str = "HIDIdleTime";

/// The console lock aggregate's key, MATCHED QUOTED so the key's own token is
/// what the search anchors on rather than any name that merely ends in it.
const IOREG_LOCK_KEY: &str = "\"IOConsoleLocked\"";

/// Absolute, because a probe must not resolve a system binary through a PATH
/// it does not control.
pub const IOREG_PATH: &str = "/usr/sbin/ioreg";
pub const PGREP_PATH: &str = "/usr/bin/pgrep";
pub const PS_PATH: &str = "/bin/ps";

/// Where a terminal name from `ps` resolves to. The name is validated before
/// it is joined, because this is the one place a reading becomes a PATH.
const TTY_DIR: &str = "/dev";

/// The idle nanosecond count, taken from the FIRST line that carries the key.
///
/// The registry prints one key per line as `"Key" = value`; the count is the
/// last whitespace-separated field. A line without one, or no line at all,
/// yields None, which every consumer already reads as "unknown".
///
/// Contaminated output is refused WHOLESALE rather than searched, matching the
/// bash reference's grep, which treats NUL-bearing input as binary. A
/// replacement character means the runner already substituted an invalid byte,
/// the same corruption. Trusting either can coerce to 0, which reads as
/// "actively typing" and silently drops the push.
pub fn parse_idle_nanoseconds(ioreg_output: &str) -> Option<&str> {
    if ioreg_output.contains(['\0', '\u{FFFD}']) {
        return None;
    }
    ioreg_output
        .lines()
        .find(|line| line.contains(IOREG_IDLE_KEY))
        .and_then(|line| line.split_whitespace().last())
}

/// Whether the console is locked, from the Root dictionary's own aggregate.
///
/// `Some(true)` for `Yes`, `Some(false)` for `No`, `None` for a key that is
/// absent or carries anything else. THE FAIL DIRECTION IS DELIBERATE and the
/// decision states it (`surface::surface`): only `Some(true)` locks, so a
/// reading nobody could take leaves the shipped desk-freshness behavior in
/// place instead of killing the desk banner permanently wherever this key is
/// renamed or dropped.
///
/// WHAT THE QUOTED KEY KEEPS OUT is the `"IOConsoleUsers"` line printed
/// beside it: that array holds one dictionary per login session, each with a
/// nested `CGSSessionScreenIsLocked` of its own, and reading a per-session
/// flag would mean picking the console session first. `IOConsoleLocked` is
/// the aggregate the kernel already computed.
pub fn parse_screen_locked(ioreg_output: &str) -> Option<bool> {
    match ioreg_output
        .lines()
        .find(|line| line.contains(IOREG_LOCK_KEY))?
        .split_whitespace()
        .last()?
    {
        "Yes" => Some(true),
        "No" => Some(false),
        _ => None,
    }
}

/// The process ids `pgrep` printed, one per line, discarding anything that is
/// not a plain decimal id.
///
/// The RAW line is validated, never a trimmed copy of it, because the bash
/// reference matches its regex against the raw line too: padding is output this
/// module did not expect, and trimming it away would promote garbled output
/// into a trusted process id.
pub fn parse_pids(pgrep_output: &str) -> Vec<String> {
    pgrep_output
        .lines()
        .filter(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// The terminal names `ps` printed, one per line, discarding every line that
/// is not one.
///
/// THIS IS A TRUST BOUNDARY, because the name is about to become a path under
/// `/dev`. A process with no controlling terminal prints `??`, and that is
/// only the benign case: anything not plain alphanumeric is refused outright,
/// so no reading can carry a slash or a `..` into the join below.
///
/// The name is trimmed first, unlike a process id: `ps -o tty=` pads its
/// column to a fixed width, so the padding is the format rather than the
/// garbled output that padding around a pid would be.
pub fn parse_tty_names(ps_output: &str) -> Vec<&str> {
    ps_output
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric()))
        .collect()
}

/// The tab the SESSION is showing, from `workspace list`: the active tab of
/// the one workspace flagged focused.
///
/// This is the only session-global answer herdr gives. No workspace flagged
/// focused is None, which becomes an unreadable view rather than a guess.
pub fn parse_focused_tab(workspace_list_json: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(workspace_list_json)
        .ok()?
        .pointer("/result/workspaces")?
        .as_array()?
        .iter()
        .find(|workspace| workspace.get("focused").and_then(|f| f.as_bool()) == Some(true))?
        .get("active_tab_id")?
        .as_str()
        .map(str::to_string)
}

/// One tab's arrangement, from `pane layout`.
#[derive(Debug, PartialEq)]
pub struct TabLayout {
    /// The tab this layout describes, which is the tab holding whichever pane
    /// the call addressed.
    pub tab_id: String,
    /// The focused pane WITHIN this tab. Tab-level truth, not the caller's
    /// pane: every pane in a tab is answered the same focused pane id.
    pub focused_pane: String,
    /// ZOOM IS TAB-LEVEL: one pane fills the window and every sibling is off
    /// screen.
    pub zoomed: bool,
}

/// A tab's arrangement, addressed by any pane inside it. The pane list is not
/// read: visibility turns on the focused pane and the zoom flag alone.
///
/// A missing field is a shape we do not know, and the whole reading is
/// refused rather than half-trusted: assuming a tab is unzoomed suppresses a
/// notification the operator cannot see.
pub fn parse_layout(layout_json: &str) -> Option<TabLayout> {
    let layout = serde_json::from_str::<serde_json::Value>(layout_json)
        .ok()?
        .pointer("/result/layout")?
        .clone();
    Some(TabLayout {
        tab_id: layout.get("tab_id")?.as_str()?.to_string(),
        focused_pane: layout.get("focused_pane_id")?.as_str()?.to_string(),
        zoomed: layout.get("zoomed")?.as_bool()?,
    })
}

/// One of this tool's state files read back whole, or the reason it was
/// refused: nothing at the path, something there that is not a regular file,
/// too large to pull into memory, or bytes no reader can decode.
///
/// EVERY READER OF THESE FILES GOES THROUGH IT, the prune's read-back, the
/// doctor's two sections and the presence probe alike, because a raw `read_to_string` on a path an
/// operator, a backup tool or another program can reach is the same two bugs
/// wherever it is written. A FIFO parks the open forever, for READING as much
/// as for writing, which wedges the hook that appended or the command a human
/// is waiting on. A file some other hand grew to gigabytes is otherwise
/// learned about by allocating it.
///
/// `symlink_metadata`, so the link itself is judged rather than whatever it
/// points at, matching the append's own refusal a few lines up. The SIZE IS
/// CHECKED FIRST for the reason above; `read_max` is the CALLER'S ceiling, far
/// above anything that caller writes and far below a size worth reading, so
/// only a file some other hand left there can reach it.
///
/// THE REFUSALS ARE `io::Error`s rather than an absence, so a caller that has
/// to tell "there is no file" from "the file could not be read" still can:
/// the doctor says a different sentence for each, and the prune heals on
/// either.
pub fn readable_state_file(path: &std::path::Path, read_max: u64) -> std::io::Result<String> {
    let found = std::fs::symlink_metadata(path)?;
    if !found.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the state file is not a regular file",
        ));
    }
    if found.len() > read_max {
        return Err(std::io::Error::new(
            std::io::ErrorKind::FileTooLarge,
            "the state file is larger than this reads",
        ));
    }
    std::fs::read_to_string(path)
}

/// What the desk thread hands back on join: the idle reading, and the lock
/// reading only where idle parsed. See `join_desk`.
type DeskHandle = std::thread::JoinHandle<(Option<u64>, Option<bool>)>;

/// The five probes: four read the machine through commands, and the marker
/// reads the filesystem directly, because an mtime needs no subprocess.
///
/// One struct rather than five, because they share the runner and a caller
/// composes the traits it needs. SOLID: the command probes depend on the
/// runner abstraction, never on `Command` directly, so that edge substitutes
/// in tests; the marker's substitution point is the path it is handed.
/// ONE PROBE SET IS ONE READING, however many consumers ask for it. Each
/// reading is taken at most once and remembered, including the reading that
/// came back empty, because an unreadable probe is an answer too.
///
/// The blocked path is what makes this load-bearing: it asks where the
/// operator is twice by design, once to decide whether an approval is
/// forwarded to the phone at all and again to decide what the notification
/// delivers. Taking the measurement twice lets a freshness boundary fall
/// between them, which cards a phone with no round trip behind it.
pub struct SystemProbes<R: CommandRunner> {
    runner: Arc<R>,
    marker_path: String,
    /// Where a phone reading's terminal name resolves to. Always `TTY_DIR`
    /// in production; a test points it at a fixture directory instead of
    /// stubbing `newest_terminal_atime` a second time, see `with_tty_dir`.
    tty_dir: String,
    idle: std::cell::OnceCell<Option<u64>>,
    marker_mtime: std::cell::OnceCell<Option<u64>>,
    phone_atime: std::cell::OnceCell<Option<u64>>,
    screen_locked: std::cell::OnceCell<Option<bool>>,
    now: std::cell::OnceCell<Option<u64>>,
    /// Set by `start` and taken by the first read that needs it: see
    /// `ProbeStart` and `join_desk`. `None` means either nothing was ever
    /// started, or a thread already ran and was already joined.
    desk_handle: std::cell::Cell<Option<DeskHandle>>,
    /// The phone twin of `desk_handle`.
    phone_handle: std::cell::Cell<Option<std::thread::JoinHandle<Option<u64>>>>,
}

impl<R: CommandRunner> SystemProbes<R> {
    pub fn new(runner: R, marker_path: String) -> Self {
        Self {
            runner: Arc::new(runner),
            marker_path,
            tty_dir: TTY_DIR.to_string(),
            idle: std::cell::OnceCell::new(),
            marker_mtime: std::cell::OnceCell::new(),
            phone_atime: std::cell::OnceCell::new(),
            screen_locked: std::cell::OnceCell::new(),
            now: std::cell::OnceCell::new(),
            desk_handle: std::cell::Cell::new(None),
            phone_handle: std::cell::Cell::new(None),
        }
    }

    /// The wall clock, taken once and remembered like the four probes beside
    /// it: see the struct doc. THE FIFTH MEMOIZED READING. Of the four beside
    /// it, only the phone atime and the marker mtime are epochs aged against
    /// this clock; idle is already an age and screen lock is a boolean. Which
    /// is why forwarding to the phone and deciding what to deliver must read
    /// this same cell rather than each taking their own: a second boundary
    /// between two wall-clock reads is what drifted a phone reading and a
    /// desk reading apart in R4-1. AN UNREADABLE CLOCK IS REMEMBERED TOO: the
    /// first reader's `None` is the second reader's `None`, so the two can
    /// never disagree about whether there was a clock at all.
    pub fn now_secs(&self) -> Option<u64> {
        *self.now.get_or_init(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|since_epoch| since_epoch.as_secs())
        })
    }

    /// Seeds the clock reading for a test, so a suite can pin "now" to an
    /// exact second instead of racing the real one.
    ///
    /// `cfg(test)`-ONLY AND NEVER FROM THE ENVIRONMENT. The four probes
    /// beside this one stay live in production no matter what a test does,
    /// because nothing here reads an override out of `std::env`: unlike the
    /// desk and phone thresholds, the wall clock has no operator-facing knob
    /// to seed it by accident.
    #[cfg(test)]
    pub fn with_clock(self, now: u64) -> Self {
        let _ = self.now.set(Some(now));
        self
    }

    /// Points the phone chain's terminal lookup at a fixture directory
    /// instead of `TTY_DIR`, so a test's canned `ps` output can name a
    /// device the test itself created and stamped, rather than a real
    /// `/dev` entry whose presence varies by machine.
    ///
    /// `cfg(test)`-ONLY, same as `with_clock` beside it: production always
    /// takes the `TTY_DIR` default set in `new`.
    #[cfg(test)]
    pub fn with_tty_dir(mut self, dir: String) -> Self {
        self.tty_dir = dir;
        self
    }
}

/// The idle probe's own body, free of `&self` so `start` can run it on a
/// spawned thread against a cloned `Arc<R>` rather than borrowing the struct
/// across threads. The trait impl below runs the SAME function inline.
fn idle_reading<R: CommandRunner>(runner: &R) -> Option<u64> {
    let ioreg_output = runner.run(IOREG_PATH, &["-c", "IOHIDSystem"])?;
    crate::presence::idle_secs_from_ns(parse_idle_nanoseconds(&ioreg_output)?)
}

/// The lock probe's own body, same reason as `idle_reading` beside it.
///
/// The Root node with its own properties, which is the one node carrying
/// the console aggregate; `-d1` stops the walk there rather than printing
/// the tree under it.
///
/// A SECOND `ioreg` SPAWN, not a second parse of the idle probe's output:
/// that one asks for the `IOHIDSystem` class and the aggregate is not in
/// it. This read is the cheaper of the two by a wide margin (92KB against
/// 294KB, measured on dresden 2026-08-28) and only happens where the idle
/// reading it exists to qualify was taken.
fn lock_reading<R: CommandRunner>(runner: &R) -> Option<bool> {
    parse_screen_locked(&runner.run(IOREG_PATH, &["-n", "Root", "-d1"])?)
}

impl<R: CommandRunner + Send + Sync + 'static> crate::probes::IdleProbe for SystemProbes<R> {
    fn idle_secs(&self) -> Option<u64> {
        self.join_desk();
        *self.idle.get_or_init(|| idle_reading(&*self.runner))
    }
}

impl<R: CommandRunner + Send + Sync + 'static> crate::probes::ScreenLockProbe for SystemProbes<R> {
    fn screen_locked(&self) -> Option<bool> {
        self.join_desk();
        *self
            .screen_locked
            .get_or_init(|| lock_reading(&*self.runner))
    }
}

impl<R: CommandRunner> crate::probes::PhoneMarkerProbe for SystemProbes<R> {
    fn marker_mtime_secs(&self) -> Option<u64> {
        *self.marker_mtime.get_or_init(|| {
            // The LINK itself, never its target, matching BSD `stat -f %m`: the
            // Back Tap touch lands on this path, so a dangling link still
            // carries the reading and following it would erase one.
            let modified = std::fs::symlink_metadata(&self.marker_path)
                .ok()?
                .modified()
                .ok()?;
            Some(
                modified
                    .duration_since(std::time::UNIX_EPOCH)
                    .ok()?
                    .as_secs(),
            )
        })
    }
}

/// The phone chain's own body, free of `&self` for the same reason as
/// `idle_reading` above: `start` runs it on a spawned thread.
///
/// WHEN THE PHONE LAST TYPED, SCROLLED OR TAPPED INTO THE SESSION, as the
/// access time of the pty its mosh client is attached to.
///
/// THE READING IS ATIME, AND THAT IS THE WHOLE TRICK. On macOS a tty's
/// atime moves when something is written INTO it and its mtime moves when
/// something is read OUT of it, so atime is input and mtime is the agent
/// talking back. Proven live on 2026-08-15 in both directions: a scroll on
/// the phone moved the mosh pty's atime while typing at the desk left it
/// untouched. That is what makes this comparable with the desk's own idle
/// clock instead of the byte sample it replaces, which passive viewing
/// could not move at all.
///
/// THREE BOUNDED SPAWNS, never one per process: `mosh-server` runs
/// detached with no controlling terminal of its own, so the terminal
/// belongs to the client it forked, and both `pgrep -P` and `ps -p` take
/// the whole list of ids at once.
///
/// FRESHEST WINS across every session found, and any step coming back
/// empty leaves None. None is never fresh, which drops the phone out of
/// the arbitration rather than parking the operator on it: a phone that
/// cannot be read must not silence the banner.
fn phone_reading<R: CommandRunner>(runner: &R, tty_dir: &str) -> Option<u64> {
    let servers = parse_pids(&runner.run(PGREP_PATH, &["-x", "mosh-server"])?);
    let clients = parse_pids(&pgrep_children(runner, &servers)?);
    if clients.is_empty() {
        return None;
    }
    let terminals = runner.run(PS_PATH, &["-o", "tty=", "-p", &clients.join(",")])?;
    newest_terminal_atime(tty_dir, &terminals)
}

/// Every child of the given parents, in one call. No parents means no call:
/// `pgrep -P` with an empty list is a usage error, not a query answering
/// "none".
fn pgrep_children<R: CommandRunner>(runner: &R, parents: &[String]) -> Option<String> {
    if parents.is_empty() {
        return None;
    }
    runner.run(PGREP_PATH, &["-P", &parents.join(",")])
}

impl<R: CommandRunner + Send + Sync + 'static> crate::probes::PhoneInputProbe for SystemProbes<R> {
    fn phone_input_atime_secs(&self) -> Option<u64> {
        self.join_phone();
        *self
            .phone_atime
            .get_or_init(|| phone_reading(&*self.runner, &self.tty_dir))
    }
}

/// The most recent access time among the terminals `ps` named, or None when
/// not one of them could be read.
///
/// The directory is a parameter so the lookup can be pointed at fixtures; in
/// production it is always `/dev`.
pub fn newest_terminal_atime(tty_dir: &str, ps_output: &str) -> Option<u64> {
    parse_tty_names(ps_output)
        .into_iter()
        .filter_map(|name| atime_secs(&format!("{tty_dir}/{name}")))
        .max()
}

/// A file's access time in whole seconds since the epoch, or None when it
/// cannot be read. A plain `stat`, which does not itself count as an access,
/// so taking the reading never disturbs it.
fn atime_secs(path: &str) -> Option<u64> {
    Some(
        std::fs::metadata(path)
            .ok()?
            .accessed()
            .ok()?
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs(),
    )
}

impl<R: CommandRunner> crate::probes::SessionViewProbe for SystemProbes<R> {
    /// Two reads, and NEITHER may be caller-relative.
    ///
    /// `herdr pane current` is the trap this builder exists to avoid: it
    /// resolves "current" from the CALLER'S `HERDR_PANE_ID`, and the caller
    /// is always the pane the event fired from, so a view built on it makes
    /// the origin its own focused pane and every desk event self-suppresses.
    /// Measured live on 2026-08-13 (drill D4): with the session zoomed onto
    /// wW:p3R, a hook in wW:p3K was answered wW:p3K.
    ///
    /// So what is on screen comes from `workspace list`, the one session-
    /// global answer herdr gives, and the arrangement comes from the ORIGIN
    /// tab's own layout, addressed by the pane id the event carried. That
    /// layout names the origin's tab as well, so the third call is gone.
    /// Either call failing yields None, which the model reads as Unknown
    /// rather than as "not visible".
    ///
    /// NO CELL, UNLIKE THE OTHER FOUR PROBES ON THIS STRUCT: this has exactly
    /// one production reader (`engine::operator_visibility`), so "one probe
    /// set is one reading" already holds by call site alone, with nothing to
    /// memoize against. A second production reader would need the same
    /// `OnceCell` the other four carry, to keep that property true once it is
    /// no longer free.
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView> {
        let focused_tab = parse_focused_tab(&self.herdr("workspace", &["list"])?)?;
        let layout = parse_layout(&self.herdr("pane", &["layout", "--pane", origin_pane])?)?;
        Some(crate::surface::SessionView {
            origin_tab: layout.tab_id,
            focused_tab,
            focused_pane: layout.focused_pane,
            zoomed: layout.zoomed,
        })
    }
}

impl<R: CommandRunner> SystemProbes<R> {
    /// Resolved through PATH, unlike the system binaries above: the
    /// multiplexer is not at a fixed location, and a context whose PATH does
    /// not carry it reads as unknown, which fails OPEN into a notification.
    fn herdr(&self, subcommand: &str, args: &[&str]) -> Option<String> {
        let mut argv = vec![subcommand];
        argv.extend_from_slice(args);
        self.runner.run("herdr", &argv)
    }
}

impl<R: CommandRunner + Send + Sync + 'static> crate::probes::ProbeStart for SystemProbes<R> {
    /// Begin the desk pair and the phone chain in the background, one thread
    /// each: see the module doc on `SystemProbes` and `join_desk`/`join_phone`
    /// below. NEITHER OVERRIDE IS CONSULTED HERE; the caller already answered
    /// that in `wants`, which is the one spelling of the override rule this
    /// and the read guards in `engine::surface_reading` share.
    ///
    /// EVERY THREAD STARTED HERE IS JOINED BY A READ ON THE SAME PATH before
    /// anything calls `std::env::set_var`: the guards in `surface_reading`
    /// read exactly what they asked to start, so no probe thread outlives
    /// that function, and the one `set_var` in this crate (main's blocked
    /// path) runs after it returns. `set_var` is `unsafe` because libc
    /// readers such as `localtime_r` do not take std's environment lock;
    /// `Command::spawn` does, so the rule is the general contract of a
    /// multi-threaded process, not a spawn race. Keep it when adding a
    /// thread or a `set_var`.
    fn start(&self, wants: crate::probes::Wants) {
        if wants.desk && self.idle.get().is_none() && self.desk_handle_absent() {
            let runner = Arc::clone(&self.runner);
            // CAPTURED BEFORE THE SPAWN, not read from inside the thread: a
            // caller that already read the lock inline (nothing in
            // production does, but nothing forbade it either) filled
            // `screen_locked` before `start` ever ran, and the thread must
            // not run `ioreg -n Root -d1` a second time for an answer
            // `join_desk`'s `OnceCell::set` would only discard.
            let lock_already_known = self.screen_locked.get().is_some();
            let handle = std::thread::Builder::new()
                .spawn(move || {
                    // THE LOCK RIDES THIS THREAD RATHER THAN ITS OWN: the
                    // engine's rule is "the lock is read only where idle
                    // answered" (see `join_desk`), so running it here, gated
                    // on the SAME idle result, is what keeps a failed idle
                    // read from spawning a second `ioreg` for an answer
                    // nothing can use.
                    let idle = idle_reading(&*runner);
                    let lock = (!lock_already_known && idle.is_some())
                        .then(|| lock_reading(&*runner))
                        .flatten();
                    (idle, lock)
                })
                // A THREAD THE OS REFUSES FALLS BACK TO THE INLINE READ: `ok()`
                // drops a spawn failure into "nothing started", which is
                // exactly the state `join_desk` and the trait impls already
                // treat as "compute it when asked".
                .ok();
            self.desk_handle.set(handle);
        }
        if wants.phone && self.phone_atime.get().is_none() && self.phone_handle_absent() {
            let runner = Arc::clone(&self.runner);
            let tty_dir = self.tty_dir.clone();
            let handle = std::thread::Builder::new()
                .spawn(move || phone_reading(&*runner, &tty_dir))
                .ok();
            self.phone_handle.set(handle);
        }
    }
}

impl<R: CommandRunner + Send + Sync + 'static> SystemProbes<R> {
    /// Whether a desk thread is currently in flight, without consuming the
    /// handle: `Cell` has no borrow for a value that is not `Copy`, so
    /// peeking means taking it out and setting it straight back, which is
    /// safe because only the one owning thread ever touches this cell (see
    /// the crate's `OnceCell is !Sync` note: the struct itself stays
    /// single-threaded, only the probe bodies run elsewhere).
    fn desk_handle_absent(&self) -> bool {
        let handle = self.desk_handle.take();
        let absent = handle.is_none();
        self.desk_handle.set(handle);
        absent
    }

    /// The phone twin of `desk_handle_absent`.
    fn phone_handle_absent(&self) -> bool {
        let handle = self.phone_handle.take();
        let absent = handle.is_none();
        self.phone_handle.set(handle);
        absent
    }

    /// Join the desk thread if `start` began one, filling BOTH cells it owns
    /// from that one join. A no-op wherever nothing was started: the trait
    /// impls' own `get_or_init` then computes inline exactly as before this
    /// existed, which is what makes a caller that never starts answer
    /// exactly as it always has.
    ///
    /// FILLING BOTH CELLS TOGETHER, even when the lock was never attempted
    /// (idle failed to parse), is what keeps a later `screen_locked()` read
    /// from spawning a second `ioreg` for an answer the thread already
    /// decided nothing could give: the cell holds `None` either way, and
    /// `None` already means "no reading" everywhere this crate reads it.
    fn join_desk(&self) {
        if let Some(handle) = self.desk_handle.take() {
            let (idle, lock) = handle
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
            let _ = self.idle.set(idle);
            let _ = self.screen_locked.set(lock);
        }
    }

    /// The phone twin of `join_desk`.
    fn join_phone(&self) {
        if let Some(handle) = self.phone_handle.take() {
            let atime = handle
                .join()
                .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
            let _ = self.phone_atime.set(atime);
        }
    }
}

/// Minutes since LOCAL midnight for an epoch second, or None when the system
/// cannot say.
///
/// THE ONE PLACE the local zone is read. Which hour an epoch second falls in
/// is a system fact (a zone database, a `TZ` variable and two transitions a
/// year), not a calculation, so it is asked of libc rather than derived here,
/// and the answer leaves as a plain number: every rule about quiet hours is a
/// value function over this minute, with no clock inside it.
pub fn local_minutes_since_midnight(epoch_secs: u64) -> Option<u16> {
    let seconds = libc::time_t::try_from(epoch_secs).ok()?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `localtime_r` writes the broken-down time into the `tm` it is
    // handed and returns either that same pointer or null. `seconds` points
    // at a live `time_t` on this frame, `broken_down` is an aligned `tm` this
    // frame owns for the whole call and nothing else aliases, and the
    // reentrant form writes its answer only into that buffer, which is what
    // makes it the thread-safe one to call from here. The buffer is read ONLY
    // after a non-null return, which is what proves it was initialized.
    let local = unsafe {
        if libc::localtime_r(&seconds, broken_down.as_mut_ptr()).is_null() {
            return None;
        }
        broken_down.assume_init()
    };
    // Range-checked rather than trusted: this is an FFI boundary, and a minute
    // of day is what every caller is promised.
    u16::try_from(local.tm_hour.checked_mul(60)?.checked_add(local.tm_min)?)
        .ok()
        .filter(|minutes| *minutes < 1440)
}

/// One epoch second as an RFC 3339 instant in UTC, or None when the system
/// cannot say.
///
/// UTC AND NOT THE LOCAL ZONE, which is the whole reason this is a second
/// function rather than a format applied to the first. The only caller states a
/// window to a REMOTE search service, and a bare local time would be read there
/// as an hour the operator did not mean, twice a year by a different amount.
/// The `Z` is what makes the instant unambiguous wherever it is parsed.
pub fn utc_timestamp(epoch_secs: u64) -> Option<String> {
    let seconds = libc::time_t::try_from(epoch_secs).ok()?;
    let mut broken_down = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: `gmtime_r`'s contract is `localtime_r`'s above, and for the same
    // reasons: `seconds` points at a live `time_t` on this frame, `broken_down`
    // is an aligned `tm` this frame owns for the whole call with nothing else
    // aliasing it, the reentrant form writes only into that buffer, and the
    // buffer is read ONLY after a non-null return, which is what proves it was
    // initialized.
    let utc = unsafe {
        if libc::gmtime_r(&seconds, broken_down.as_mut_ptr()).is_null() {
            return None;
        }
        broken_down.assume_init()
    };
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        utc.tm_year.checked_add(1900)?,
        utc.tm_mon.checked_add(1)?,
        utc.tm_mday,
        utc.tm_hour,
        utc.tm_min,
        utc.tm_sec
    ))
}

#[cfg(test)]
mod tests {
    use super::{
        CommandRunner, FIRST_POLL_INTERVAL, POLL_INTERVAL, local_minutes_since_midnight,
        newest_terminal_atime, next_poll_interval, parse_focused_tab, parse_idle_nanoseconds,
        parse_layout, parse_pids, parse_screen_locked, parse_tty_names, run_bounded, utc_timestamp,
        wait_until,
    };
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn the_wait_between_checks_doubles_and_stops_at_the_ceiling() {
        // The ceiling is what keeps a wedged child from being polled thousands
        // of times a second for the whole deadline; the doubling is what keeps
        // the ordinary case, a child already exiting, off a flat 10ms bill.
        assert_eq!(
            next_poll_interval(FIRST_POLL_INTERVAL),
            FIRST_POLL_INTERVAL * 2
        );
        // AND THE FIRST INTERVAL HAS TO GROW, which doubling alone does not
        // give: zero doubles to zero, so a first interval of nothing is a
        // `try_wait` spin for the whole deadline rather than the backoff the
        // ceiling above was written to guarantee.
        assert!(
            next_poll_interval(FIRST_POLL_INTERVAL) > FIRST_POLL_INTERVAL,
            "a wait that starts at zero never leaves it"
        );
        assert_eq!(next_poll_interval(POLL_INTERVAL / 2), POLL_INTERVAL);
        assert_eq!(next_poll_interval(POLL_INTERVAL), POLL_INTERVAL);
        assert!(FIRST_POLL_INTERVAL < POLL_INTERVAL, "it starts below it");
    }

    /// How many steps of the schedule the test below watches: enough to pass
    /// the ceiling, which is where a call site that doubles without capping
    /// parts company with one that does not.
    const WATCHED_STEPS: usize = 8;

    #[test]
    fn the_wait_sleeps_the_schedule_it_computes_rather_than_a_flat_ceiling() {
        // THE HELPER ABOVE IS NOT THE FIX. A correct backoff computed beside a
        // loop that still sleeps `POLL_INTERVAL` leaves every assertion up
        // there green and every bounded spawn paying the flat bill again, so
        // what is pinned here is the LINE THAT SLEEPS: the durations the loop
        // hands its sleeper, in order, against the schedule the helper states.
        //
        // NOTHING SLEEPS AND NOTHING IS TIMED. The fake sleeper returns at
        // once, so the child is alive for exactly as many polls as it allows
        // and dropping its stdin is what ends the wait. `cat` holds that pipe
        // open until it does.
        let mut child = std::process::Command::new("/bin/cat")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("a child to wait on");
        let mut stdin = child.stdin.take();
        let mut slept: Vec<Duration> = Vec::new();
        wait_until(
            &mut child,
            std::time::Instant::now() + Duration::from_secs(5),
            |interval| {
                slept.push(interval);
                if slept.len() >= WATCHED_STEPS {
                    drop(stdin.take());
                }
            },
        );
        // DERIVED, so the expectation cannot drift away from the constants: it
        // is the schedule `next_poll_interval` defines, walked from the first
        // interval. What the test states is that the loop walks it too.
        let schedule: Vec<Duration> = std::iter::successors(Some(FIRST_POLL_INTERVAL), |current| {
            Some(next_poll_interval(*current))
        })
        .take(WATCHED_STEPS)
        .collect();
        assert!(
            slept.len() >= WATCHED_STEPS,
            "the wait polled {} times, too few to show a schedule",
            slept.len()
        );
        assert_eq!(
            &slept[..WATCHED_STEPS],
            schedule.as_slice(),
            "the loop slept its own schedule"
        );
    }

    /// One child writing `bytes` zeroes, read under a 4096-byte ceiling.
    ///
    /// THE ONLY TESTS HERE THAT SPAWN, and they have to: what they pin is the
    /// reader, and the reader only exists around a real pipe. Every other test
    /// in this module drives fixture text through a parser, because the parsers
    /// are the part a suite must not take off the live machine.
    fn read_under_a_4096_byte_cap(bytes: usize) -> Option<String> {
        let mut command = std::process::Command::new("/bin/sh");
        command.args(["-c", &format!("head -c {bytes} /dev/zero; exit 0")]);
        run_bounded(command, None, Duration::from_secs(5), 4096)
    }

    #[test]
    fn a_command_that_talks_past_the_cap_is_no_answer_rather_than_a_truncated_one() {
        // WITHOUT THE BOUND THIS READS THE LOT. A `read_to_end` into a growing
        // `Vec` is bounded by the DEADLINE alone, so a child that streams for
        // its whole window hands back everything it wrote and the caller learns
        // the size only once it is all in memory. The summarizer is exactly
        // that child: an operator-named command running a model for minutes.
        //
        // AND A TRUNCATED ANSWER IS WORSE THAN NO ANSWER, which is why the cap
        // refuses in the DEADLINE'S OWN DIRECTION rather than handing back what
        // it managed to read. Cut at the ceiling, a process list loses its last
        // rows and a JSON listing stops mid-object, and both arrive looking
        // exactly like a complete short answer. Every caller reads no answer as
        // unknown, and unknown never suppresses.
        assert_eq!(read_under_a_4096_byte_cap(100_000), None);
    }

    #[test]
    fn a_short_answer_is_told_apart_from_the_cap_because_it_is_still_an_answer() {
        // The other half of the same statement: the refusal above has to be the
        // CAP and not the reader giving up on anything large-ish, so a child
        // that stops on its own is read whole, and the ceiling itself is a
        // working answer rather than the first refused one.
        assert_eq!(
            read_under_a_4096_byte_cap(12).map(|read| read.len()),
            Some(12)
        );
        assert_eq!(
            read_under_a_4096_byte_cap(4096).map(|read| read.len()),
            Some(4096),
            "the ceiling is inclusive, as every other bound in this crate is"
        );
    }

    /// Records what it was asked to run and answers from a script, so a test
    /// pins both the parsing and the exact argv a probe uses.
    struct FakeRunner {
        answers: Vec<(String, Option<String>)>,
        calls: Mutex<Vec<String>>,
    }

    impl FakeRunner {
        fn answering(answer: &str) -> Self {
            // The empty key matches every program, so one answer serves any
            // single-command probe.
            Self {
                answers: vec![(String::new(), Some(answer.to_string()))],
                calls: Mutex::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                answers: Vec::new(),
                calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("{program} {}", args.join(" ")));
            self.answers
                .iter()
                .find(|(key, _)| program.contains(key.as_str()))
                .and_then(|(_, answer)| answer.clone())
        }
    }

    // --- one reading per probe set ------------------------------------------

    /// Counts what it was asked to run, and keeps the counter reachable after
    /// the runner is handed to the probe set that owns it.
    ///
    /// `Arc<AtomicU32>`, not `Rc<Cell<u32>>`: `start` hands a clone of this
    /// runner to a spawned thread, which needs `Send + Sync`, and neither
    /// `Rc` nor `Cell` is either.
    struct CountingRunner {
        answer: String,
        calls: Arc<std::sync::atomic::AtomicU32>,
    }

    impl CommandRunner for CountingRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Option<String> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Some(self.answer.clone())
        }
    }

    #[test]
    fn a_reading_asked_for_twice_is_still_taken_once() {
        // The blocked path asks where the operator is TWICE by design: once to
        // decide whether an approval is forwarded to the phone at all, and
        // again to decide what the notification delivers. Two spawns can
        // answer differently, and a freshness boundary crossed between them
        // cards a phone with no round trip behind it.
        //
        // INLINE, START-FREE, ON PURPOSE (C5): a started desk thread makes two
        // runner calls by design (idle, then the lock it qualifies), so this
        // stays a plain read to keep pinning "no start, no thread, one call".
        use crate::probes::IdleProbe;
        let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let probes = SystemProbes::new(
            CountingRunner {
                answer: "\"HIDIdleTime\" = 5000000000".to_string(),
                calls: Arc::clone(&calls),
            },
            "/nonexistent/marker".to_string(),
        );
        assert_eq!(probes.idle_secs(), Some(5));
        assert_eq!(
            probes.idle_secs(),
            Some(5),
            "and the same answer both times"
        );
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one probe set is one reading"
        );
    }

    #[test]
    fn a_reading_that_came_back_empty_is_not_retaken_either() {
        // An unreadable probe is an ANSWER, and re-taking it would let two
        // consumers disagree about a machine that told the first one nothing.
        use crate::probes::IdleProbe;
        let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let probes = SystemProbes::new(
            CountingRunner {
                answer: "nothing the parser recognizes".to_string(),
                calls: Arc::clone(&calls),
            },
            "/nonexistent/marker".to_string(),
        );
        assert_eq!(probes.idle_secs(), None);
        assert_eq!(probes.idle_secs(), None);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn starting_twice_and_reading_twice_spawns_each_probe_once() {
        // PRESERVATION (C6): the concurrent path answers no differently from
        // the sequential one it replaces, however many times `start` and the
        // reads race each other. `forward_to_moshi` then `run_event` both
        // call `start` on the SAME probe set (main.rs:1939, :2383), and this
        // is that shape: start, read every probe, start again, read every
        // probe again, every worker joined by the time the last read
        // returns.
        use crate::probes::{ProbeStart, ScreenLockProbe, Wants};
        let mut scripted: Vec<(String, String)> = DISCOVERY
            .iter()
            .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
            .collect();
        scripted.push((
            "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
            "\"HIDIdleTime\" = 5000000000".to_string(),
        ));
        scripted.push((
            "/usr/sbin/ioreg -n Root -d1".to_string(),
            ROOT_LOCKED.to_string(),
        ));
        // DISCOVERY's canned `ps` output names "ttys000", a real `/dev`
        // entry only on a machine with an open terminal by that name. A CI
        // runner has none, so the phone join reads None there against
        // TTY_DIR and this test's own "joined value" assertion goes flaky
        // by host rather than by behavior. The fixture directory below is
        // what `with_tty_dir` points the phone chain at instead, so the
        // assertion is a real number every machine can produce.
        const JOINED_PHONE_ATIME: u64 = 1_650_000_000;
        let tty_dir = std::env::temp_dir().join(format!("pns-tty-join-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tty_dir);
        std::fs::create_dir_all(&tty_dir).expect("fixture dir");
        terminal_with_atime(&tty_dir, "ttys000", JOINED_PHONE_ATIME);
        let probes = SystemProbes::new(
            ExactArgvRunner {
                answers: scripted,
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        )
        .with_tty_dir(tty_dir.to_string_lossy().into_owned());
        for _ in 0..2 {
            probes.start(Wants {
                desk: true,
                phone: true,
            });
            probes.idle_secs();
            probes.screen_locked();
            probes.phone_input_atime_secs();
        }
        let calls = probes.runner.calls.lock().unwrap();
        for expected in [
            "/usr/sbin/ioreg -c IOHIDSystem",
            "/usr/sbin/ioreg -n Root -d1",
            "/usr/bin/pgrep -x mosh-server",
            "/usr/bin/pgrep -P 14362",
            "/bin/ps -o tty= -p 14363",
        ] {
            assert_eq!(
                calls.iter().filter(|call| *call == expected).count(),
                1,
                "case: {expected}, calls were {calls:?}"
            );
        }
        drop(calls);
        // sol review, ROW 2: the assertions above only count runner calls;
        // a `join_desk`/`join_phone` that stored `None` for a successful
        // worker passed every one of them. Assert the joined values too.
        assert_eq!(probes.idle_secs(), Some(5), "the joined idle answer");
        assert_eq!(probes.screen_locked(), Some(true), "the joined lock answer");
        let phone_reading = probes.phone_input_atime_secs();
        let _ = std::fs::remove_dir_all(&tty_dir);
        assert_eq!(
            phone_reading,
            Some(JOINED_PHONE_ATIME),
            "the joined phone answer"
        );
    }

    #[test]
    fn a_lock_probe_already_answered_inline_is_not_retaken_by_a_later_start() {
        // sol review, ROW 1: `start` gated the desk spawn on the idle cell
        // only, so a caller reading the lock BEFORE starting the desk pair
        // ran `ioreg -n Root -d1` a second time on the spawned thread, and
        // `join_desk`'s `OnceCell::set` silently dropped that second answer.
        // No production caller reads in this order, but nothing enforced it.
        use crate::probes::{ProbeStart, ScreenLockProbe, Wants};
        let probes = SystemProbes::new(
            ExactArgvRunner {
                answers: vec![
                    (
                        "/usr/sbin/ioreg -n Root -d1".to_string(),
                        ROOT_LOCKED.to_string(),
                    ),
                    (
                        "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
                        "\"HIDIdleTime\" = 5000000000".to_string(),
                    ),
                ],
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        );
        assert_eq!(probes.screen_locked(), Some(true));
        probes.start(Wants {
            desk: true,
            phone: false,
        });
        assert_eq!(probes.idle_secs(), Some(5));
        let calls = probes.runner.calls.lock().unwrap();
        assert_eq!(
            calls
                .iter()
                .filter(|call| *call == "/usr/sbin/ioreg -n Root -d1")
                .count(),
            1,
            "a lock answer already taken inline must not be retaken: {calls:?}"
        );
    }

    #[test]
    fn a_desk_only_start_spawns_no_phone_thread_and_the_phone_still_reads_inline() {
        // sol review, ROW 4: no deterministic test exercised a one-sided
        // `Wants`. A production `start` that ignored `phone: false` and
        // spawned the phone chain anyway would pass every other test here.
        use crate::probes::{PhoneInputProbe, ProbeStart, Wants};
        let mut scripted: Vec<(String, String)> = DISCOVERY
            .iter()
            .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
            .collect();
        scripted.push((
            "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
            "\"HIDIdleTime\" = 5000000000".to_string(),
        ));
        let probes = SystemProbes::new(
            ExactArgvRunner {
                answers: scripted,
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        );
        probes.start(Wants {
            desk: true,
            phone: false,
        });
        probes.idle_secs(); // joins the desk thread so its calls have landed
        let phone_calls_before_the_read = probes
            .runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("/usr/bin/pgrep") || call.starts_with("/bin/ps"))
            .count();
        assert_eq!(
            phone_calls_before_the_read, 0,
            "a desk-only start must not touch the phone chain"
        );
        probes.phone_input_atime_secs();
        let phone_calls_after_the_read = probes
            .runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("/usr/bin/pgrep") || call.starts_with("/bin/ps"))
            .count();
        assert_eq!(
            phone_calls_after_the_read, 3,
            "the later phone read computes inline, exactly as an unstarted read always has"
        );
    }

    #[test]
    fn a_phone_only_start_spawns_no_desk_thread_and_the_desk_still_reads_inline() {
        // The mirror of the test above: a production `start` that ignored
        // `desk: false` and spawned the desk pair anyway during a phone-only
        // read would pass every other test here.
        use crate::probes::{IdleProbe, ProbeStart, Wants};
        let mut scripted: Vec<(String, String)> = DISCOVERY
            .iter()
            .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
            .collect();
        scripted.push((
            "/usr/sbin/ioreg -c IOHIDSystem".to_string(),
            "\"HIDIdleTime\" = 5000000000".to_string(),
        ));
        let probes = SystemProbes::new(
            ExactArgvRunner {
                answers: scripted,
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        );
        probes.start(Wants {
            desk: false,
            phone: true,
        });
        probes.phone_input_atime_secs(); // joins the phone thread so its calls have landed
        let desk_calls_before_the_read = probes
            .runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("/usr/sbin/ioreg"))
            .count();
        assert_eq!(
            desk_calls_before_the_read, 0,
            "a phone-only start must not touch the desk pair"
        );
        probes.idle_secs();
        let desk_calls_after_the_read = probes
            .runner
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.starts_with("/usr/sbin/ioreg"))
            .count();
        assert_eq!(
            desk_calls_after_the_read, 1,
            "the later desk read computes inline, exactly as an unstarted read always has"
        );
    }

    #[test]
    fn the_clock_is_the_fifth_memoized_reading() {
        // A wall clock read twice can answer twice: a second boundary between
        // the two reads is exactly what drifted a phone reading and a desk
        // reading apart in R4-1. Seeding a fixed value and asking twice is
        // what proves this answers the CELL and not the clock: a mutant that
        // bypassed the cell would answer the real epoch here, not 42.
        let probes = probes_answering("unused").with_clock(42);
        assert_eq!(probes.now_secs(), Some(42));
        assert_eq!(
            probes.now_secs(),
            Some(42),
            "and the same answer both times"
        );
    }

    // --- parse_idle_nanoseconds --------------------------------------------

    #[test]
    fn the_idle_count_is_the_last_field_of_the_line_that_carries_the_key() {
        let output = "    | |   \"HIDIdleTime\" = 5000000000\n    | |   \"Other\" = 1\n";
        assert_eq!(parse_idle_nanoseconds(output), Some("5000000000"));
    }

    #[test]
    fn the_first_idle_line_wins_so_a_second_device_cannot_override_it() {
        let output = "\"HIDIdleTime\" = 5000000000\n\"HIDIdleTime\" = 99\n";
        assert_eq!(parse_idle_nanoseconds(output), Some("5000000000"));
    }

    #[test]
    fn output_without_the_idle_key_reads_as_unknown_rather_than_zero() {
        assert_eq!(parse_idle_nanoseconds("\"Other\" = 1\n"), None);
        assert_eq!(parse_idle_nanoseconds(""), None);
    }

    #[test]
    fn contaminated_idle_output_reads_as_unknown_rather_than_a_reading() {
        // The bash reference (grep, in binary mode) refuses NUL-bearing output
        // outright. Trusting a corrupted stream can coerce to 0, which reads
        // as "actively typing" and silently suppresses the push; U+FFFD is
        // where the runner replaced invalid bytes, the same corruption.
        assert_eq!(parse_idle_nanoseconds("\0\"HIDIdleTime\" = 0\n"), None);
        assert_eq!(
            parse_idle_nanoseconds("\u{FFFD}\"HIDIdleTime\" = 5000000000\n"),
            None
        );
    }

    // --- parse_screen_locked ------------------------------------------------

    /// The Root dictionary as `/usr/sbin/ioreg -n Root -d1` prints it, trimmed
    /// to the neighbourhood of the key.
    ///
    /// The `"IOConsoleLocked"` LINES ARE LIVE SHAPES, each captured on dresden
    /// (Darwin 25.2.0) in the state it describes: `= Yes` while the screen was
    /// genuinely locked, `= No` while it was not, six leading spaces and all.
    /// The `"IOConsoleUsers"` line beside it is the captured one with the
    /// nested `CGSSessionScreenIsLocked` WRITTEN IN. That key is the decoy
    /// this fixture aims at rather than a claim about what the kernel prints
    /// next to a locked console, and what it pins is the parser reading the
    /// aggregate off its own line instead of anything in that per-session
    /// array.
    const ROOT_LOCKED: &str = r#"+-o Root  <class IORegistryEntry, id 0x100000100, retain 28>
    {
      "OS Build Version" = "25C56"
      "IOConsoleLocked" = Yes
      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes,"kCGSSessionUserNameKey"="stephen"})
    }
"#;

    /// The unlocked shape carrying the same decoy: a parser reaching into the
    /// per-session array reads Yes here and reports a locked screen at a desk
    /// the operator is sitting at.
    ///
    /// THE DECOY LINE COMES FIRST ON PURPOSE. The parser answers from the
    /// FIRST line carrying the key, so a search string loose enough to match
    /// the per-session flag has to meet that flag before the aggregate for
    /// this fixture to catch it. Put the aggregate back on top and the test
    /// below passes for any search string at all.
    const ROOT_UNLOCKED_WITH_DECOY: &str = r#"+-o Root  <class IORegistryEntry, id 0x100000100, retain 28>
    {
      "OS Build Version" = "25C56"
      "IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"CGSSessionScreenIsLocked"=Yes,"kCGSSessionUserNameKey"="stephen"})
      "IOConsoleLocked" = No
    }
"#;

    #[test]
    fn the_console_key_saying_yes_is_a_locked_screen() {
        assert_eq!(parse_screen_locked(ROOT_LOCKED), Some(true));
    }

    #[test]
    fn the_console_key_saying_no_is_an_unlocked_screen_whatever_the_session_array_says() {
        // THE PRECISION TEST. `CGSSessionScreenIsLocked` rides inside the
        // `"IOConsoleUsers"` line one row ABOVE, where the parser meets it
        // first, so a parser that searches for anything less specific than the
        // aggregate's own key answers from a per-session flag and reports a
        // locked screen at an occupied desk.
        assert_eq!(parse_screen_locked(ROOT_UNLOCKED_WITH_DECOY), Some(false));
    }

    #[test]
    fn a_console_key_that_is_missing_or_says_something_else_reads_as_no_reading() {
        // None is not "unlocked", it is "nobody could tell", and only the
        // decision above states what to do about that. Reporting either
        // verdict here would put a guess where a reading belongs.
        assert_eq!(
            parse_screen_locked("+-o Root\n    {\n      \"OS Build Version\" = \"25C56\"\n    }\n"),
            None,
            "the key is not in this dictionary at all"
        );
        assert_eq!(parse_screen_locked(""), None, "and no output is no reading");
        assert_eq!(
            parse_screen_locked("      \"IOConsoleLocked\" = Maybe\n"),
            None,
            "a value this parser does not know is not a verdict"
        );
        assert_eq!(
            parse_screen_locked("      \"IOConsoleLocked\"\n"),
            None,
            "and neither is a key printed with no value beside it"
        );
    }

    // --- parse_pids ---------------------------------------------------------

    #[test]
    fn every_decimal_id_is_kept_one_per_line() {
        assert_eq!(parse_pids("101\n2002\n"), vec!["101", "2002"]);
    }

    #[test]
    fn a_line_that_is_not_a_plain_id_is_discarded_rather_than_passed_to_the_sampler() {
        assert_eq!(
            parse_pids("101\nnot-a-pid\n-5\n\n2002\n"),
            vec!["101", "2002"]
        );
    }

    #[test]
    fn no_sessions_at_all_is_an_empty_list_and_never_an_error() {
        assert!(parse_pids("").is_empty());
    }

    #[test]
    fn a_padded_pid_line_is_rejected_like_any_other_malformed_line() {
        // The bash reference validates the raw line; trimming first would
        // promote garbled output into a trusted process id.
        assert_eq!(parse_pids(" 101 \n2002\n"), vec!["2002"]);
    }

    // --- parse_focused_pane -------------------------------------------------

    // --- the production runner, against real processes ----------------------

    use super::SystemCommandRunner;

    #[test]
    fn the_production_runner_captures_stdout_on_success() {
        assert_eq!(
            SystemCommandRunner.run("/bin/echo", &["ok"]),
            Some("ok\n".to_string())
        );
    }

    #[test]
    fn the_production_runner_yields_no_reading_from_a_failing_command() {
        // Partial output from a failed command must never be parsed as a live
        // reading; None is the unknown every consumer fails safe on.
        assert_eq!(SystemCommandRunner.run("/usr/bin/false", &[]), None);
    }

    #[test]
    fn the_production_runner_yields_no_reading_for_a_missing_binary() {
        assert_eq!(
            SystemCommandRunner.run("/nonexistent/pns-no-such-binary", &[]),
            None
        );
    }

    #[test]
    fn the_production_runner_keeps_a_reading_with_stray_invalid_bytes() {
        // Every reading is judged line by line downstream, so one bad byte
        // must cost its own line rather than the whole answer.
        let out = SystemCommandRunner
            .run("/bin/sh", &["-c", "printf 'a\\377b'"])
            .expect("stray bytes must not discard the reading");
        assert!(out.starts_with('a') && out.ends_with('b'));
    }

    // --- the five probe implementations, the behavior R2a owes -------------

    use super::SystemProbes;
    use crate::probes::{IdleProbe, PhoneInputProbe, PhoneMarkerProbe, SessionViewProbe};
    use crate::surface::{Visibility, visibility};

    fn probes_answering(answer: &str) -> SystemProbes<FakeRunner> {
        SystemProbes::new(FakeRunner::answering(answer), "/marker".to_string())
    }

    fn probes_failing() -> SystemProbes<FakeRunner> {
        SystemProbes::new(FakeRunner::failing(), "/marker".to_string())
    }

    #[test]
    fn the_idle_probe_reports_whole_seconds_from_the_nanosecond_count() {
        let probes = probes_answering("\"HIDIdleTime\" = 5000000000\n");
        assert_eq!(probes.idle_secs(), Some(5));
    }

    #[test]
    fn an_idle_command_that_fails_reports_unknown_which_fails_open_into_a_push() {
        assert_eq!(probes_failing().idle_secs(), None);
    }

    #[test]
    fn a_garbled_idle_count_is_unknown_rather_than_zero_seconds_idle() {
        // Zero would read as "actively typing" and silently drop the push.
        let probes = probes_answering("\"HIDIdleTime\" = not-a-number\n");
        assert_eq!(probes.idle_secs(), None);
    }

    #[test]
    fn the_marker_probe_reports_the_files_modification_time_in_whole_seconds() {
        // The marker is read straight off the filesystem, no subprocess: a
        // freshly written file's mtime must land within seconds of now, and
        // the bound is two-sided so a future mtime cannot read as fresh.
        let path = std::env::temp_dir().join(format!("pns-marker-test-{}", std::process::id()));
        std::fs::write(&path, b"").unwrap();
        let probes = SystemProbes::new(FakeRunner::failing(), path.to_string_lossy().into_owned());
        let reading = probes.marker_mtime_secs();
        std::fs::remove_file(&path).ok();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let mtime = reading.expect("a marker that exists must yield a reading");
        assert!(now.abs_diff(mtime) <= 5);
    }

    #[test]
    fn an_absent_marker_reports_unknown_which_the_marker_rule_fails_closed_on() {
        let probes = SystemProbes::new(
            FakeRunner::failing(),
            "/nonexistent/pns-marker-test".to_string(),
        );
        assert_eq!(probes.marker_mtime_secs(), None);
    }

    #[test]
    fn the_marker_probe_reads_the_link_itself_never_its_target() {
        // BSD stat -f %m reads the link, and the Back Tap touch lands on the
        // path itself: a dangling link still has its own mtime, so following
        // it to a missing target must not erase the reading.
        let link = std::env::temp_dir().join(format!("pns-marker-link-{}", std::process::id()));
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("pns-nonexistent-target", &link).unwrap();
        let probes = SystemProbes::new(FakeRunner::failing(), link.to_string_lossy().into_owned());
        let reading = probes.marker_mtime_secs();
        std::fs::remove_file(&link).ok();
        assert!(
            reading.is_some(),
            "the dangling link's own mtime is the reading"
        );
    }

    // --- the phone's input clock -------------------------------------------

    /// The probe with the three discovery answers scripted by exact argv,
    /// pointed at a marker path nothing reads.
    fn phone_probe(answers: &[(&str, &str)]) -> SystemProbes<ExactArgvRunner> {
        SystemProbes::new(
            ExactArgvRunner {
                answers: answers
                    .iter()
                    .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
                    .collect(),
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        )
    }

    /// The live chain, recorded on dresden 2026-08-15: one detached
    /// `mosh-server`, the herdr client it forked, and that client's pty.
    const DISCOVERY: [(&str, &str); 3] = [
        ("/usr/bin/pgrep -x mosh-server", "14362\n"),
        ("/usr/bin/pgrep -P 14362", "14363\n"),
        // ps pads its column to a fixed width, which the parser trims.
        ("/bin/ps -o tty= -p 14363", "ttys000 \n"),
    ];

    #[test]
    fn the_discovery_argv_is_pinned_to_the_chain_that_was_measured_live() {
        // THREE CALLS, in this order, with the ids batched rather than one
        // spawn per process. `mosh-server` itself has no controlling
        // terminal (measured: `??`), which is why the client is walked to at
        // all; a reordering or a dropped step ships a probe that silently
        // never reads a phone.
        let probes = phone_probe(&DISCOVERY);
        probes.phone_input_atime_secs();
        assert_eq!(
            probes.runner.calls.lock().unwrap().as_slice(),
            &[
                "/usr/bin/pgrep -x mosh-server".to_string(),
                "/usr/bin/pgrep -P 14362".to_string(),
                "/bin/ps -o tty= -p 14363".to_string(),
            ]
        );
    }

    #[test]
    fn every_server_and_every_client_is_asked_for_in_one_call_each() {
        // Two phones attached at once is the case that grows the id lists,
        // and both pgrep and ps take the whole list, so the spawn count
        // stays at three however many sessions are open.
        let probes = phone_probe(&[
            ("/usr/bin/pgrep -x mosh-server", "14362\n900\n"),
            ("/usr/bin/pgrep -P 14362,900", "14363\n901\n"),
            ("/bin/ps -o tty= -p 14363,901", "ttys000 \nttys001 \n"),
        ]);
        probes.phone_input_atime_secs();
        assert_eq!(probes.runner.calls.lock().unwrap().len(), 3);
    }

    #[test]
    fn a_failure_at_any_step_of_the_chain_reads_as_no_phone_rather_than_a_fresh_one() {
        // Never fresh is the fail direction: a phone that cannot be read
        // must drop out of the arbitration, not park the operator on it and
        // silence every banner. Each case drops one scripted answer, which
        // is that command failing.
        for dropped in [
            "/usr/bin/pgrep -x mosh-server",
            "/usr/bin/pgrep -P 14362",
            "/bin/ps -o tty= -p 14363",
        ] {
            let scripted: Vec<(&str, &str)> = DISCOVERY
                .iter()
                .copied()
                .filter(|(call, _)| *call != dropped)
                .collect();
            assert_eq!(
                phone_probe(&scripted).phone_input_atime_secs(),
                None,
                "case: {dropped} unanswered"
            );
        }
    }

    #[test]
    fn no_mosh_server_at_all_never_asks_for_children_of_nothing() {
        // `pgrep -P` with an empty list is a usage error, not a query
        // answering "none", so the walk stops rather than spawning it.
        let probes = phone_probe(&[("/usr/bin/pgrep -x mosh-server", "\n")]);
        assert_eq!(probes.phone_input_atime_secs(), None);
        assert_eq!(
            probes.runner.calls.lock().unwrap().as_slice(),
            &["/usr/bin/pgrep -x mosh-server".to_string()]
        );
    }

    #[test]
    fn a_server_whose_client_has_no_terminal_reads_as_no_phone() {
        // `ps -o tty=` prints `??` for a process with no controlling
        // terminal, and there is no clock on a terminal that is not there.
        let probes = phone_probe(&[
            ("/usr/bin/pgrep -x mosh-server", "14362\n"),
            ("/usr/bin/pgrep -P 14362", "14363\n"),
            ("/bin/ps -o tty= -p 14363", "??       \n"),
        ]);
        assert_eq!(probes.phone_input_atime_secs(), None);
    }

    // --- parse_tty_names, the step that becomes a path ----------------------

    #[test]
    fn a_padded_terminal_name_is_trimmed_because_the_padding_is_the_format() {
        // Unlike a pid line, where padding is output we did not expect, `ps
        // -o tty=` pads its column by design.
        assert_eq!(
            parse_tty_names("ttys000 \nttys001  \n"),
            ["ttys000", "ttys001"]
        );
    }

    #[test]
    fn a_process_with_no_controlling_terminal_names_none() {
        assert!(parse_tty_names("??       \n").is_empty());
        assert!(parse_tty_names("").is_empty());
        assert_eq!(parse_tty_names("??      \nttys000 \n"), ["ttys000"]);
    }

    #[test]
    fn a_name_that_could_escape_the_device_directory_is_refused_outright() {
        // The name is joined onto /dev, so this is the trust boundary: a
        // reading carrying a slash or a dot-dot must never become a path.
        for hostile in [
            "../../etc/passwd",
            "..",
            "tty/../../root",
            "tty s000",
            "tty;rm",
            "tty.0",
        ] {
            assert!(
                parse_tty_names(&format!("{hostile}\n")).is_empty(),
                "case: {hostile}"
            );
        }
    }

    // --- newest_terminal_atime, against files whose atimes are set ----------

    /// A file whose access time is exactly this many seconds past the epoch.
    ///
    /// AN ABSOLUTE INSTANT, never a wall-clock stamp. This used to shell out
    /// to `touch -a -t`, which reads its stamp in the HOST'S LOCAL TIME, so
    /// the fixture meant one epoch on the developer's machine and another on
    /// a UTC runner: the same two assertions passed in Denver and failed in
    /// CI, seven hours apart. The probe under test reports epoch seconds, so
    /// the fixture states epoch seconds and the assertion reads the same
    /// constant back.
    fn terminal_with_atime(dir: &std::path::Path, name: &str, atime_secs: u64) {
        let file = std::fs::File::create(dir.join(name)).expect("terminal fixture");
        file.set_times(
            std::fs::FileTimes::new()
                .set_accessed(std::time::UNIX_EPOCH + std::time::Duration::from_secs(atime_secs)),
        )
        .expect("set the fixture atime");
    }

    /// Two instants far enough apart that nothing but the freshest can win.
    const PUT_DOWN_ATIME: u64 = 1_577_836_800;
    const IN_HAND_ATIME: u64 = 1_609_459_200;

    #[test]
    fn the_freshest_terminal_wins_across_every_session_found() {
        // Two phones attached, one put down an hour ago and one in a hand:
        // the reading is the one being used, so the stale session cannot
        // drag the verdict away from the live one.
        let dir = std::env::temp_dir().join(format!("pns-tty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture dir");
        terminal_with_atime(&dir, "ttys000", PUT_DOWN_ATIME);
        terminal_with_atime(&dir, "ttys001", IN_HAND_ATIME);
        let newest = newest_terminal_atime(&dir.to_string_lossy(), "ttys000 \nttys001 \n");
        let stale = newest_terminal_atime(&dir.to_string_lossy(), "ttys000 \n");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            newest,
            Some(IN_HAND_ATIME),
            "the newer atime is the reading"
        );
        assert_eq!(stale, Some(PUT_DOWN_ATIME), "and alone the older one is");
    }

    #[test]
    fn a_terminal_that_cannot_be_stat_ed_drops_out_without_taking_the_others_with_it() {
        let dir = std::env::temp_dir().join(format!("pns-tty-gone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("fixture dir");
        terminal_with_atime(&dir, "ttys000", PUT_DOWN_ATIME);
        let mixed = newest_terminal_atime(&dir.to_string_lossy(), "ttysGONE \nttys000 \n");
        let none = newest_terminal_atime(&dir.to_string_lossy(), "ttysGONE \n");
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(mixed, Some(PUT_DOWN_ATIME));
        assert_eq!(none, None, "nothing readable is no reading at all");
    }

    #[test]
    fn the_idle_probe_argv_matches_the_bash_original() {
        let probes = probes_answering("\"HIDIdleTime\" = 5000000000\n");
        probes.idle_secs();
        assert_eq!(
            probes.runner.calls.lock().unwrap()[0],
            "/usr/sbin/ioreg -c IOHIDSystem"
        );
    }

    #[test]
    fn the_lock_probe_reads_the_root_dictionary_by_exact_argv_and_only_once() {
        // The Root node with its own properties, which is where the console
        // aggregate is printed; `-d1` keeps it to that one node. Once per
        // invocation, like every reading here: the blocked path asks where
        // the operator is twice by design and both answers must be the same
        // measurement.
        use crate::probes::ScreenLockProbe;
        let probes = SystemProbes::new(
            ExactArgvRunner {
                answers: vec![(
                    "/usr/sbin/ioreg -n Root -d1".to_string(),
                    ROOT_LOCKED.to_string(),
                )],
                calls: Mutex::new(Vec::new()),
            },
            "/marker".to_string(),
        );
        assert_eq!(probes.screen_locked(), Some(true));
        assert_eq!(
            probes.screen_locked(),
            Some(true),
            "and the same answer both times"
        );
        assert_eq!(
            *probes.runner.calls.lock().unwrap(),
            vec!["/usr/sbin/ioreg -n Root -d1".to_string()],
            "the exact argv, taken once"
        );
    }

    #[test]
    fn the_lock_is_not_spawned_where_idle_failed() {
        // The desk thread's own body only reads the lock where idle parsed
        // (`start`'s doc), so a failed idle must leave the lock cell filled
        // from that SAME join rather than answered by a second `ioreg`
        // spawn when `screen_locked` is read afterward: the trait-level
        // `lock_reads == 0` assertion beside this one counts calls to the
        // METHOD, never spawns, and this is the one that counts the spawn.
        use crate::probes::{IdleProbe, ProbeStart, ScreenLockProbe, Wants};
        let calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let probes = SystemProbes::new(
            CountingRunner {
                answer: "nothing the parser recognizes".to_string(),
                calls: Arc::clone(&calls),
            },
            "/nonexistent/marker".to_string(),
        );
        probes.start(Wants {
            desk: true,
            phone: false,
        });
        assert_eq!(probes.idle_secs(), None);
        assert_eq!(
            probes.screen_locked(),
            None,
            "never attempted, not merely unreadable"
        );
        assert_eq!(
            calls.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "one ioreg spawn total: the desk thread never asked for the lock, \
             and the later read found the cell the join already filled"
        );
    }

    /// A runner whose `ioreg -c IOHIDSystem` blocks until the phone chain's
    /// first `pgrep` releases it, or 2 s pass: see
    /// `a_slow_probe_does_not_hold_up_a_fast_one` (C4).
    struct GateRunner {
        release: std::sync::mpsc::Sender<()>,
        // MUTEX-WRAPPED SOLELY FOR `Sync`: an `mpsc::Receiver` is `Send` but
        // never `Sync` (it has exactly one consumer by design), and `Arc<R>`
        // needs `R: Sync` to cross into the spawned thread. Only the desk
        // thread ever locks this, once.
        wait: Mutex<std::sync::mpsc::Receiver<()>>,
        idle_answer: String,
    }

    impl CommandRunner for GateRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            let call = format!("{program} {}", args.join(" "));
            match call.as_str() {
                "/usr/sbin/ioreg -c IOHIDSystem" => {
                    // A CONCURRENT phone thread's own `pgrep` releases this;
                    // a sequential, desk-only, or join-at-start mutant never
                    // reaches that `pgrep` before this deadline. GREEN NEVER
                    // WAITS ON IT, so it is sized for a starved test thread on
                    // a loaded runner rather than for a fast red.
                    self.wait
                        .lock()
                        .unwrap()
                        .recv_timeout(Duration::from_secs(2))
                        .ok()?;
                    Some(self.idle_answer.clone())
                }
                "/usr/bin/pgrep -x mosh-server" => {
                    let _ = self.release.send(());
                    Some("14362\n".to_string())
                }
                "/usr/sbin/ioreg -n Root -d1" => Some(ROOT_LOCKED.to_string()),
                "/usr/bin/pgrep -P 14362" => Some("14363\n".to_string()),
                "/bin/ps -o tty= -p 14363" => Some("ttys000 \n".to_string()),
                _ => None,
            }
        }
    }

    #[test]
    fn a_slow_probe_does_not_hold_up_a_fast_one() {
        // PROVEN BY ORDER, NEVER BY TIME (C4). Concurrent: the phone
        // thread's `pgrep` releases the desk thread's blocked `ioreg`
        // within microseconds, so idle reads its fixture value and this
        // test returns at once. A sequential, desk-only, or
        // join-at-start mutant never starts the phone thread before
        // blocking on idle, so `ioreg` times out at 2 s into no reading
        // at all, and the assertion below is what turns that into red.
        use crate::probes::{IdleProbe, PhoneInputProbe, ProbeStart, ScreenLockProbe, Wants};
        let (release, wait) = std::sync::mpsc::channel();
        let probes = SystemProbes::new(
            GateRunner {
                release,
                wait: Mutex::new(wait),
                idle_answer: "\"HIDIdleTime\" = 5000000000".to_string(),
            },
            "/marker".to_string(),
        );
        probes.start(Wants {
            desk: true,
            phone: true,
        });
        // PRODUCTION ORDER: idle, then the lock it qualifies, then phone.
        let idle = probes.idle_secs();
        let _ = probes.screen_locked();
        let _ = probes.phone_input_atime_secs();
        assert_eq!(
            idle,
            Some(5),
            "the phone thread's own pgrep released the blocked ioreg"
        );
    }

    // --- the session view, against herdr's real answers ---------------------

    /// Recorded from a live herdr on 2026-08-13, trimmed to the fields the
    /// view needs. A shape change upstream fails these rather than silently
    /// reading Unknown forever.
    ///
    /// A workspace's `focused` flag and the `active_tab_id` beside it are the
    /// only SESSION-GLOBAL statement of what is on screen. Every `pane`
    /// answer is relative to the process that asked.
    const WORKSPACE_LIST: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wW:t9","focused":true,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;
    /// The same recorded shape with a second workspace ahead of the focused
    /// one, which is where the operator is looking.
    const WORKSPACE_LIST_SECOND_FOCUSED: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wV:t1","focused":false,"label":"other","workspace_id":"wV"},{"active_tab_id":"wW:t9","focused":true,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;
    const WORKSPACE_LIST_NONE_FOCUSED: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"active_tab_id":"wW:t9","focused":false,"label":"dotfiles modernization","workspace_id":"wW"}]}}"#;

    /// The D4 live capture: tab wW:t9 zoomed onto wW:p3R, taken while the
    /// operator held that zoom and the hook fired from wW:p3K.
    const LAYOUT_ZOOMED_ON_SIBLING: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3R","panes":[{"focused":false,"pane_id":"wW:p3K"},{"focused":true,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":true},"type":"pane_layout"}}"#;
    /// The same tab zoomed onto wW:p3K instead.
    const LAYOUT_ZOOMED_ON_ORIGIN: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":true},"type":"pane_layout"}}"#;
    const LAYOUT_UNZOOMED: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":false},"type":"pane_layout"}}"#;
    /// A pane sitting in one of the workspace's other tabs.
    const LAYOUT_OTHER_TAB: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p10","panes":[{"focused":true,"pane_id":"wW:p10"}],"tab_id":"wW:tF","zoomed":false},"type":"pane_layout"}}"#;
    /// A pane in the OTHER workspace's active tab.
    const LAYOUT_OTHER_WORKSPACE: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wV:p1","panes":[{"focused":true,"pane_id":"wV:p1"}],"tab_id":"wV:t1","zoomed":false},"type":"pane_layout"}}"#;

    /// WHAT `herdr pane current` ACTUALLY ANSWERS A HOOK, recorded live on
    /// 2026-08-13. It resolves "current" from the CALLER'S `HERDR_PANE_ID`,
    /// so a hook running inside wW:p3K is told wW:p3K, `focused` flag and
    /// all, while the session was really zoomed onto wW:p3R.
    const PANE_CURRENT_CALLER_RELATIVE: &str = r#"{"id":"cli:pane:current","result":{"pane":{"focused":false,"pane_id":"wW:p3K","tab_id":"wW:t9","workspace_id":"wW"},"type":"pane_current"}}"#;

    /// Answers exact argv and records every call, so an unscripted call reads
    /// as that herdr subcommand failing rather than as a silent default.
    struct ExactArgvRunner {
        answers: Vec<(String, String)>,
        calls: Mutex<Vec<String>>,
    }

    impl CommandRunner for ExactArgvRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            let call = format!("{program} {}", args.join(" "));
            self.calls.lock().unwrap().push(call.clone());
            self.answers
                .iter()
                .find(|(scripted, _)| *scripted == call)
                .map(|(_, answer)| answer.clone())
        }
    }

    fn viewer(answers: Vec<(String, String)>) -> SystemProbes<ExactArgvRunner> {
        SystemProbes::new(
            ExactArgvRunner {
                answers,
                calls: Mutex::new(Vec::new()),
            },
            String::new(),
        )
    }

    /// Every answer a view of `origin` could want, INCLUDING the two
    /// caller-relative ones: a test that tells the two readings apart has to
    /// let the wrong reading succeed rather than fail for want of an answer.
    fn answers(workspace_list: &str, origin: &str, layout: &str) -> Vec<(String, String)> {
        vec![
            (
                "herdr workspace list".to_string(),
                workspace_list.to_string(),
            ),
            (
                format!("herdr pane layout --pane {origin}"),
                layout.to_string(),
            ),
            (
                "herdr pane current".to_string(),
                PANE_CURRENT_CALLER_RELATIVE.to_string(),
            ),
            (
                format!("herdr pane get {origin}"),
                PANE_CURRENT_CALLER_RELATIVE.to_string(),
            ),
        ]
    }

    #[test]
    fn a_zoom_onto_a_sibling_hides_the_origin_that_pane_current_would_call_focused() {
        // THE D4 LIVE FAILURE. `herdr pane current` is CALLER-RELATIVE: the
        // hook runs inside the origin pane, so it is told the origin pane is
        // the current one, the origin therefore equals the focused pane, and
        // every desk event self-suppresses. The session was zoomed onto
        // wW:p3R the whole time.
        let view = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_SIBLING))
            .session_view("wW:p3K")
            .expect("a readable view");
        assert_eq!(view.focused_pane, "wW:p3R");
        assert_eq!(visibility("wW:p3K", &view), Visibility::Hidden);
    }

    #[test]
    fn the_view_asks_the_session_what_is_focused_and_never_asks_for_the_current_pane() {
        // The two calls that carry no caller context: the focused workspace's
        // active tab, and the ORIGIN tab's own layout, addressed by the pane
        // id the event itself carried.
        let probes = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_SIBLING));
        probes.session_view("wW:p3K").expect("a readable view");
        assert_eq!(
            probes.runner.calls.lock().unwrap().as_slice(),
            &[
                "herdr workspace list".to_string(),
                "herdr pane layout --pane wW:p3K".to_string(),
            ]
        );
    }

    #[test]
    fn the_zoomed_pane_itself_stays_visible() {
        let view = viewer(answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_ZOOMED_ON_ORIGIN))
            .session_view("wW:p3K")
            .expect("a readable view");
        assert_eq!(visibility("wW:p3K", &view), Visibility::Visible);
    }

    #[test]
    fn an_unzoomed_sibling_is_visible_beside_the_focused_pane() {
        let view = viewer(answers(WORKSPACE_LIST, "wW:p3R", LAYOUT_UNZOOMED))
            .session_view("wW:p3R")
            .expect("a readable view");
        assert_eq!(visibility("wW:p3R", &view), Visibility::Visible);
    }

    #[test]
    fn a_pane_on_another_tab_is_hidden_however_that_tab_is_arranged() {
        let view = viewer(answers(WORKSPACE_LIST, "wW:p10", LAYOUT_OTHER_TAB))
            .session_view("wW:p10")
            .expect("a readable view");
        assert_eq!(view.origin_tab, "wW:tF");
        assert_eq!(visibility("wW:p10", &view), Visibility::Hidden);
    }

    #[test]
    fn the_focused_workspace_decides_the_tab_not_the_first_one_listed() {
        // wV is listed first and its active tab holds the origin, but the
        // operator is looking at wW. Reading the first workspace instead of
        // the focused one would call this Visible.
        let view = viewer(answers(
            WORKSPACE_LIST_SECOND_FOCUSED,
            "wV:p1",
            LAYOUT_OTHER_WORKSPACE,
        ))
        .session_view("wV:p1")
        .expect("a readable view");
        assert_eq!(view.focused_tab, "wW:t9");
        assert_eq!(visibility("wV:p1", &view), Visibility::Hidden);
    }

    #[test]
    fn a_session_with_no_focused_workspace_is_unreadable_rather_than_a_guess() {
        assert!(
            viewer(answers(
                WORKSPACE_LIST_NONE_FOCUSED,
                "wW:p3K",
                LAYOUT_UNZOOMED
            ))
            .session_view("wW:p3K")
            .is_none()
        );
    }

    #[test]
    fn any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing() {
        // Unknown never suppresses, so a multiplexer that cannot answer costs
        // a spare notification rather than a lost one.
        for dropped in ["herdr workspace list", "herdr pane layout --pane wW:p3K"] {
            let scripted = answers(WORKSPACE_LIST, "wW:p3K", LAYOUT_UNZOOMED)
                .into_iter()
                .filter(|(call, _)| call != dropped)
                .collect();
            assert!(
                viewer(scripted).session_view("wW:p3K").is_none(),
                "case: {dropped} unanswered"
            );
        }
    }

    #[test]
    fn an_answer_this_parser_does_not_recognise_is_unreadable_too() {
        assert_eq!(parse_focused_tab("not json"), None);
        assert_eq!(parse_focused_tab(r#"{"result":{"workspaces":[]}}"#), None);
        // A focused workspace with no active tab names no tab, and inventing
        // one would suppress against a tab that is not on screen.
        assert_eq!(
            parse_focused_tab(r#"{"result":{"workspaces":[{"focused":true}]}}"#),
            None
        );
        assert!(parse_layout("not json").is_none());
        // A layout missing the zoom flag or either id is a shape we do not
        // know: refusing beats assuming a tab is unzoomed and suppressing.
        assert!(
            parse_layout(r#"{"result":{"layout":{"focused_pane_id":"wW:p3K","tab_id":"wW:t9"}}}"#)
                .is_none()
        );
        assert!(parse_layout(r#"{"result":{"layout":{"zoomed":false}}}"#).is_none());
    }

    /// 2025-08-24T01:46:40Z, months from the nearest daylight-saving
    /// transition of the zones this suite runs in (2025-03-09 and 2025-11-02
    /// on the developer's, none at all on a UTC runner), so the minute after
    /// it is a minute later there.
    const AUGUST_INSTANT: u64 = 1_756_000_000;

    #[test]
    fn the_local_clock_reads_a_minute_of_the_day_for_the_second_it_was_given() {
        let minutes = local_minutes_since_midnight(AUGUST_INSTANT).expect("a readable local zone");
        assert!(
            minutes < 1440,
            "a minute of the day, whatever the zone: {minutes}"
        );
        let later =
            local_minutes_since_midnight(AUGUST_INSTANT + 60).expect("a readable local zone");
        assert_eq!(
            (later + 1440 - minutes) % 1440,
            1,
            "and it reads the second it was handed, not the wall clock"
        );
    }

    #[test]
    fn the_utc_instant_is_the_same_second_wherever_the_suite_runs() {
        // THE ZONE IS NOT READ, which is the property: the only caller states a
        // window to a remote search, and a local hour would be read there as an
        // hour nobody meant. Pinned as an exact string, so a build that
        // reached for the local zone fails on the developer's machine and on a
        // UTC runner alike.
        assert_eq!(
            utc_timestamp(AUGUST_INSTANT).as_deref(),
            Some("2025-08-24T01:46:40Z")
        );
    }
}
