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
        true => wait_until(&mut child, expires_at),
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
fn wait_until(
    child: &mut std::process::Child,
    expires_at: std::time::Instant,
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
        std::thread::sleep(interval);
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
/// taken microseconds later and a flat 10ms is what a run pays for missing it.
/// MEASURED on dresden 2026-09-01, forty interleaved runs a lane: a Stop hook
/// takes seven probe spawns plus the branch lookup through here, and its
/// decision path fell from a 129ms median and a 168ms 90th percentile to 98ms
/// and 102ms.
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
    runner: R,
    marker_path: String,
    idle: std::cell::OnceCell<Option<u64>>,
    marker_mtime: std::cell::OnceCell<Option<u64>>,
    phone_atime: std::cell::OnceCell<Option<u64>>,
    screen_locked: std::cell::OnceCell<Option<bool>>,
}

impl<R: CommandRunner> SystemProbes<R> {
    pub fn new(runner: R, marker_path: String) -> Self {
        Self {
            runner,
            marker_path,
            idle: std::cell::OnceCell::new(),
            marker_mtime: std::cell::OnceCell::new(),
            phone_atime: std::cell::OnceCell::new(),
            screen_locked: std::cell::OnceCell::new(),
        }
    }
}

impl<R: CommandRunner> crate::probes::IdleProbe for SystemProbes<R> {
    fn idle_secs(&self) -> Option<u64> {
        *self.idle.get_or_init(|| {
            let ioreg_output = self.runner.run(IOREG_PATH, &["-c", "IOHIDSystem"])?;
            crate::presence::idle_secs_from_ns(parse_idle_nanoseconds(&ioreg_output)?)
        })
    }
}

impl<R: CommandRunner> crate::probes::ScreenLockProbe for SystemProbes<R> {
    /// The Root node with its own properties, which is the one node carrying
    /// the console aggregate; `-d1` stops the walk there rather than printing
    /// the tree under it.
    ///
    /// A SECOND `ioreg` SPAWN, not a second parse of the idle probe's output:
    /// that one asks for the `IOHIDSystem` class and the aggregate is not in
    /// it. This read is the cheaper of the two by a wide margin (92KB against
    /// 294KB, measured on dresden 2026-08-28) and only happens where the idle
    /// reading it exists to qualify was taken.
    fn screen_locked(&self) -> Option<bool> {
        *self.screen_locked.get_or_init(|| {
            parse_screen_locked(&self.runner.run(IOREG_PATH, &["-n", "Root", "-d1"])?)
        })
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

impl<R: CommandRunner> crate::probes::PhoneInputProbe for SystemProbes<R> {
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
    fn phone_input_atime_secs(&self) -> Option<u64> {
        *self.phone_atime.get_or_init(|| {
            let servers = parse_pids(&self.runner.run(PGREP_PATH, &["-x", "mosh-server"])?);
            let clients = parse_pids(&self.pgrep_children(&servers)?);
            if clients.is_empty() {
                return None;
            }
            let terminals = self
                .runner
                .run(PS_PATH, &["-o", "tty=", "-p", &clients.join(",")])?;
            newest_terminal_atime(TTY_DIR, &terminals)
        })
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
    /// Every child of the given parents, in one call. No parents means no
    /// call: `pgrep -P` with an empty list is a usage error, not a query
    /// answering "none".
    fn pgrep_children(&self, parents: &[String]) -> Option<String> {
        if parents.is_empty() {
            return None;
        }
        self.runner.run(PGREP_PATH, &["-P", &parents.join(",")])
    }

    /// Resolved through PATH, unlike the system binaries above: the
    /// multiplexer is not at a fixed location, and a context whose PATH does
    /// not carry it reads as unknown, which fails OPEN into a notification.
    fn herdr(&self, subcommand: &str, args: &[&str]) -> Option<String> {
        let mut argv = vec![subcommand];
        argv.extend_from_slice(args);
        self.runner.run("herdr", &argv)
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
    };
    use std::cell::RefCell;
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
        calls: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        fn answering(answer: &str) -> Self {
            // The empty key matches every program, so one answer serves any
            // single-command probe.
            Self {
                answers: vec![(String::new(), Some(answer.to_string()))],
                calls: RefCell::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                answers: Vec::new(),
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .borrow_mut()
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
    struct CountingRunner {
        answer: String,
        calls: std::rc::Rc<std::cell::Cell<u32>>,
    }

    impl CommandRunner for CountingRunner {
        fn run(&self, _program: &str, _args: &[&str]) -> Option<String> {
            self.calls.set(self.calls.get() + 1);
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
        use crate::probes::IdleProbe;
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let probes = SystemProbes::new(
            CountingRunner {
                answer: "\"HIDIdleTime\" = 5000000000".to_string(),
                calls: std::rc::Rc::clone(&calls),
            },
            "/nonexistent/marker".to_string(),
        );
        assert_eq!(probes.idle_secs(), Some(5));
        assert_eq!(
            probes.idle_secs(),
            Some(5),
            "and the same answer both times"
        );
        assert_eq!(calls.get(), 1, "one probe set is one reading");
    }

    #[test]
    fn a_reading_that_came_back_empty_is_not_retaken_either() {
        // An unreadable probe is an ANSWER, and re-taking it would let two
        // consumers disagree about a machine that told the first one nothing.
        use crate::probes::IdleProbe;
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let probes = SystemProbes::new(
            CountingRunner {
                answer: "nothing the parser recognizes".to_string(),
                calls: std::rc::Rc::clone(&calls),
            },
            "/nonexistent/marker".to_string(),
        );
        assert_eq!(probes.idle_secs(), None);
        assert_eq!(probes.idle_secs(), None);
        assert_eq!(calls.get(), 1);
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
                calls: RefCell::new(Vec::new()),
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
            probes.runner.calls.borrow().as_slice(),
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
        assert_eq!(probes.runner.calls.borrow().len(), 3);
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
            probes.runner.calls.borrow().as_slice(),
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
            probes.runner.calls.borrow()[0],
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
                calls: RefCell::new(Vec::new()),
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
            *probes.runner.calls.borrow(),
            vec!["/usr/sbin/ioreg -n Root -d1".to_string()],
            "the exact argv, taken once"
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
        calls: RefCell<Vec<String>>,
    }

    impl CommandRunner for ExactArgvRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            let call = format!("{program} {}", args.join(" "));
            self.calls.borrow_mut().push(call.clone());
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
                calls: RefCell::new(Vec::new()),
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
            probes.runner.calls.borrow().as_slice(),
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
