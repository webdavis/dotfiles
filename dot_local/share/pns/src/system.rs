//! The IO edge: the four probe traits implemented against the real machine.
//!
//! WHAT LIVES HERE AND WHAT DOES NOT. Everything here runs a command and hands
//! the bytes to a parser; every parser is a free function taking `&str`, so a
//! test drives fixture output and never spawns anything. The DECISIONS all live
//! in `presence` and `routing`, which is why nothing in this module compares,
//! thresholds or judges.
//!
//! The runner seam exists for the same reason: a test substitutes the command
//! output, so the suite never samples the live machine. That matters more than
//! usual here, because the rate sample takes a full second of live counters and
//! a suite that ran it would be both slow and nondeterministic.

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
/// EVERY PROBE IS BOUNDED. A wedged herdr, ioreg or nettop would otherwise
/// hold a notification open indefinitely, and the readings all have a
/// fail-direction already: no answer reads as unknown, which never suppresses.
pub struct SystemCommandRunner;

/// One window for every probe. The rate sample is the slow one at roughly a
/// second by construction (`nettop -L 2`); the rest answer in milliseconds, so
/// this is generous for them and still far short of a hang.
const PROBE_DEADLINE: Duration = Duration::from_secs(5);

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let mut command = Command::new(program);
        command.args(args);
        run_bounded(command, None, PROBE_DEADLINE)
    }
}

/// Run a command with a deadline, returning its stdout on success.
///
/// There is no wait-with-timeout in the standard library and macOS ships no
/// `timeout(1)`, so the wait happens on a thread and the child is killed when
/// the window closes. Every spawn on a notification path is bounded: the
/// notification is worth less than the turn it reports on.
pub fn run_bounded(
    mut command: Command,
    stdin_text: Option<&str>,
    deadline: Duration,
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
        // Bytes, then LOSSY: the rate CSV is judged row by row downstream, so
        // one invalid byte must cost its own row rather than the whole
        // sample, and read_to_string would refuse the lot.
        let mut output = Vec::new();
        let _ = std::io::Read::read_to_end(&mut stdout, &mut output);
        let _ = sender.send(String::from_utf8_lossy(&output).into_owned());
    });

    let output = receiver.recv_timeout(deadline).ok();
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
    status.success().then_some(output).flatten()
}

/// Poll a child to exit, up to a deadline. There is no wait-with-timeout in
/// the standard library and macOS ships no `timeout(1)`.
fn wait_until(
    child: &mut std::process::Child,
    expires_at: std::time::Instant,
) -> Option<std::process::ExitStatus> {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Err(_) => return None,
            Ok(None) => {}
        }
        if std::time::Instant::now() >= expires_at {
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

/// How often a bounded wait checks. Short enough not to add latency anyone
/// notices, long enough not to spin a core.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// The idle counter's own units, as the registry reports them.
const IOREG_IDLE_KEY: &str = "HIDIdleTime";

/// Absolute, because a probe must not resolve a system binary through a PATH
/// it does not control.
pub const IOREG_PATH: &str = "/usr/sbin/ioreg";
pub const PGREP_PATH: &str = "/usr/bin/pgrep";
pub const NETTOP_PATH: &str = "/usr/bin/nettop";

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

/// The tab a `pane get` or `pane current` answer belongs to, with the pane id
/// beside it. Anything unrecognised is None, which becomes an unreadable view.
pub fn parse_pane(pane_json: &str) -> Option<(String, String)> {
    let pane = serde_json::from_str::<serde_json::Value>(pane_json)
        .ok()?
        .pointer("/result/pane")?
        .clone();
    Some((
        pane.get("pane_id")?.as_str()?.to_string(),
        pane.get("tab_id")?.as_str()?.to_string(),
    ))
}

/// Whether the tab on screen is zoomed, from `pane layout`.
///
/// ZOOM IS TAB-LEVEL: one pane fills the window and every sibling is off
/// screen, which is the whole reason the layout has to be read for the tab
/// being LOOKED AT rather than the tab the event came from. The pane list is
/// not read: visibility turns on the focused pane and this flag alone.
pub fn parse_layout(layout_json: &str) -> Option<bool> {
    serde_json::from_str::<serde_json::Value>(layout_json)
        .ok()?
        .pointer("/result/layout/zoomed")?
        .as_bool()
}

/// The four probes: three read the machine through one command each, and the
/// marker reads the filesystem directly, because an mtime needs no subprocess.
///
/// One struct rather than four, because they share the runner and a caller
/// composes the traits it needs. SOLID: the command probes depend on the
/// runner abstraction, never on `Command` directly, so that edge substitutes
/// in tests; the marker's substitution point is the path it is handed.
pub struct SystemProbes<R: CommandRunner> {
    runner: R,
    marker_path: String,
}

impl<R: CommandRunner> SystemProbes<R> {
    pub fn new(runner: R, marker_path: String) -> Self {
        Self {
            runner,
            marker_path,
        }
    }
}

impl<R: CommandRunner> crate::probes::IdleProbe for SystemProbes<R> {
    fn idle_secs(&self) -> Option<u64> {
        let ioreg_output = self.runner.run(IOREG_PATH, &["-c", "IOHIDSystem"])?;
        crate::presence::idle_secs_from_ns(parse_idle_nanoseconds(&ioreg_output)?)
    }
}

impl<R: CommandRunner> crate::probes::PhoneMarkerProbe for SystemProbes<R> {
    fn marker_mtime_secs(&self) -> Option<u64> {
        // The LINK itself, never its target, matching BSD `stat -f %m`: the
        // Back Tap touch lands on this path, so a dangling link still carries
        // the reading and following it would erase one.
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
    }
}

impl<R: CommandRunner> crate::probes::MoshRateProbe for SystemProbes<R> {
    fn sample_csv(&self) -> Option<String> {
        let pids = parse_pids(&self.runner.run(PGREP_PATH, &["-x", "mosh-server"])?);
        if pids.is_empty() {
            return None;
        }
        // -P collapses to one row per process, -x prints raw byte counts rather
        // than MiB, -n skips address resolution, and -L 2 is what makes this two
        // samples a second apart in CSV. The -J column list puts bytes_in in
        // field 5, which is the shape the rate judge parses.
        let mut args = vec![
            "-P",
            "-L",
            "2",
            "-x",
            "-n",
            "-J",
            "time,interface,state,bytes_in,bytes_out",
        ];
        for pid in &pids {
            args.push("-p");
            args.push(pid);
        }
        self.runner.run(NETTOP_PATH, &args)
    }
}

impl<R: CommandRunner> crate::probes::SessionViewProbe for SystemProbes<R> {
    /// Three reads, because three facts live in three places: what is focused
    /// right now, what that tab contains, and which tab the event came from.
    /// Any one of them failing yields None, and the model reads that as
    /// Unknown rather than as "not visible".
    fn session_view(&self, origin_pane: &str) -> Option<crate::surface::SessionView> {
        let (focused_pane, focused_tab) = parse_pane(&self.herdr("pane", &["current"])?)?;
        let zoomed = parse_layout(&self.herdr("pane", &["layout", "--pane", &focused_pane])?)?;
        let (_, origin_tab) = parse_pane(&self.herdr("pane", &["get", origin_pane])?)?;
        Some(crate::surface::SessionView {
            origin_tab,
            focused_tab,
            focused_pane,
            zoomed,
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

#[cfg(test)]
mod tests {
    use super::{CommandRunner, parse_idle_nanoseconds, parse_layout, parse_pane, parse_pids};
    use std::cell::RefCell;

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

        /// One answer per program, keyed by a substring of its path, for the
        /// probe that chains two commands.
        fn scripted(scripts: &[(&str, &str)]) -> Self {
            Self {
                answers: scripts
                    .iter()
                    .map(|(program, out)| ((*program).to_string(), Some((*out).to_string())))
                    .collect(),
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
    fn the_production_runner_keeps_a_sample_with_stray_invalid_bytes() {
        // The rate CSV is judged row by row downstream, matching the bash awk
        // parser's tolerance: one bad byte must not discard a whole sample.
        let out = SystemCommandRunner
            .run("/bin/sh", &["-c", "printf 'a\\377b'"])
            .expect("stray bytes must not discard the reading");
        assert!(out.starts_with('a') && out.ends_with('b'));
    }

    // --- the four probe implementations, the behavior R2a owes -------------

    use super::SystemProbes;
    use crate::probes::{IdleProbe, MoshRateProbe, PhoneMarkerProbe, SessionViewProbe};
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

    #[test]
    fn no_sessions_means_no_sample_and_the_sampler_never_runs() {
        // An empty CSV would be judged INACTIVE either way, but skipping the
        // sampler is what keeps a full second of live counters off this path;
        // the call recording is the proof it was skipped.
        let probes = probes_answering("");
        assert_eq!(probes.sample_csv(), None);
        let calls = probes.runner.calls.borrow();
        assert_eq!(calls.len(), 1, "only pgrep may run, got {calls:?}");
        assert_eq!(calls[0], "/usr/bin/pgrep -x mosh-server");
    }

    #[test]
    fn a_pgrep_with_no_matches_is_no_sample_which_reads_inactive() {
        // Real pgrep exits non-zero on no matches, which the runner reports
        // as None: the shape the production path actually produces.
        assert_eq!(probes_failing().sample_csv(), None);
    }

    #[test]
    fn the_sampler_argv_matches_the_bash_original_byte_for_byte() {
        // The -J column order puts bytes_in in field 5, the exact shape the
        // rate judge parses; a reordering here ships a dead probe silently.
        let probes = SystemProbes::new(
            FakeRunner::scripted(&[("pgrep", "101\n2002\n"), ("nettop", "csv\n")]),
            "/marker".to_string(),
        );
        assert_eq!(probes.sample_csv(), Some("csv\n".to_string()));
        let calls = probes.runner.calls.borrow();
        assert_eq!(calls[0], "/usr/bin/pgrep -x mosh-server");
        assert_eq!(
            calls[1],
            "/usr/bin/nettop -P -L 2 -x -n -J time,interface,state,bytes_in,bytes_out -p 101 -p 2002"
        );
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

    // --- the session view, against herdr's real answers ---------------------

    /// Recorded from a live herdr on 2026-08-12, trimmed to the fields the
    /// view needs. A shape change upstream fails these rather than silently
    /// reading Unknown forever.
    const PANE_CURRENT: &str = r#"{"id":"cli:pane:current","result":{"pane":{"focused":true,"pane_id":"wW:p3K","tab_id":"wW:t9","workspace_id":"wW"},"type":"pane_current"}}"#;
    const PANE_GET_SAME_TAB: &str = r#"{"id":"cli:pane:get","result":{"pane":{"focused":false,"pane_id":"wW:p3R","tab_id":"wW:t9","workspace_id":"wW"},"type":"pane_info"}}"#;
    const PANE_GET_OTHER_TAB: &str = r#"{"id":"cli:pane:get","result":{"pane":{"focused":false,"pane_id":"wW:p7","tab_id":"wW:t4","workspace_id":"wW"},"type":"pane_info"}}"#;
    const LAYOUT_ZOOMED: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":true},"type":"pane_layout"}}"#;
    const LAYOUT_UNZOOMED: &str = r#"{"id":"cli:pane:layout","result":{"layout":{"focused_pane_id":"wW:p3K","panes":[{"focused":true,"pane_id":"wW:p3K"},{"focused":false,"pane_id":"wW:p3R"}],"tab_id":"wW:t9","zoomed":false},"type":"pane_layout"}}"#;

    /// A runner answering each herdr subcommand, and recording what it was
    /// asked. `None` for a subcommand is that call failing.
    struct HerdrRunner {
        current: Option<&'static str>,
        layout: Option<&'static str>,
        get: Option<&'static str>,
        calls: RefCell<Vec<String>>,
    }

    impl CommandRunner for HerdrRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
            // argv is ["pane", <verb>, ...]: the verb is what differs.
            match args.get(1) {
                Some(&"current") => self.current.map(String::from),
                Some(&"layout") => self.layout.map(String::from),
                _ => self.get.map(String::from),
            }
        }
    }

    fn viewer(
        current: Option<&'static str>,
        layout: Option<&'static str>,
        get: Option<&'static str>,
    ) -> SystemProbes<HerdrRunner> {
        SystemProbes::new(
            HerdrRunner {
                current,
                layout,
                get,
                calls: RefCell::new(Vec::new()),
            },
            String::new(),
        )
    }

    #[test]
    fn the_view_reads_the_focused_tab_and_the_origins_tab_from_herdrs_own_answers() {
        let probes = viewer(
            Some(PANE_CURRENT),
            Some(LAYOUT_UNZOOMED),
            Some(PANE_GET_SAME_TAB),
        );
        let view = probes.session_view("wW:p3R").expect("a readable view");
        assert_eq!(view.focused_tab, "wW:t9");
        assert_eq!(view.focused_pane, "wW:p3K");
        assert_eq!(view.origin_tab, "wW:t9");
        assert!(!view.zoomed);
        // The layout must be read for the pane being LOOKED AT, not the one
        // the event came from: zoom is a property of the tab on screen.
        assert_eq!(
            probes.runner.calls.borrow().as_slice(),
            &[
                "herdr pane current".to_string(),
                "herdr pane layout --pane wW:p3K".to_string(),
                "herdr pane get wW:p3R".to_string(),
            ]
        );
    }

    #[test]
    fn a_zoomed_sibling_reads_hidden_and_an_unzoomed_one_reads_visible() {
        // The two readings that differ only by the tab's zoom flag, carried
        // all the way through the model the way the engine will.
        let zoomed = viewer(
            Some(PANE_CURRENT),
            Some(LAYOUT_ZOOMED),
            Some(PANE_GET_SAME_TAB),
        )
        .session_view("wW:p3R")
        .expect("a readable view");
        assert_eq!(visibility("wW:p3R", &zoomed), Visibility::Hidden);

        let unzoomed = viewer(
            Some(PANE_CURRENT),
            Some(LAYOUT_UNZOOMED),
            Some(PANE_GET_SAME_TAB),
        )
        .session_view("wW:p3R")
        .expect("a readable view");
        assert_eq!(visibility("wW:p3R", &unzoomed), Visibility::Visible);
    }

    #[test]
    fn a_pane_on_another_tab_reads_hidden_however_the_focused_tab_is_arranged() {
        let view = viewer(
            Some(PANE_CURRENT),
            Some(LAYOUT_UNZOOMED),
            Some(PANE_GET_OTHER_TAB),
        )
        .session_view("wW:p7")
        .expect("a readable view");
        assert_eq!(visibility("wW:p7", &view), Visibility::Hidden);
    }

    #[test]
    fn any_herdr_call_failing_leaves_the_view_unreadable_rather_than_guessing() {
        // Unknown never suppresses, so a multiplexer that cannot answer costs
        // a spare notification rather than a lost one.
        for (current, layout, get) in [
            (None, Some(LAYOUT_UNZOOMED), Some(PANE_GET_SAME_TAB)),
            (Some(PANE_CURRENT), None, Some(PANE_GET_SAME_TAB)),
            (Some(PANE_CURRENT), Some(LAYOUT_UNZOOMED), None),
        ] {
            assert!(
                viewer(current, layout, get)
                    .session_view("wW:p3R")
                    .is_none()
            );
        }
    }

    #[test]
    fn an_answer_this_parser_does_not_recognise_is_unreadable_too() {
        assert_eq!(parse_pane("not json"), None);
        assert_eq!(parse_pane(r#"{"result":{}}"#), None);
        assert_eq!(parse_layout("not json"), None);
        // A layout with no zoom flag is a shape we do not know: refusing it
        // beats assuming a tab is unzoomed and suppressing a notification.
        assert_eq!(parse_layout(r#"{"result":{"layout":{"panes":[]}}}"#), None);
    }
}
