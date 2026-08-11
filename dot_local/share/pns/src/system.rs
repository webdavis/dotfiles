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

/// Runs a command and returns its stdout, or `None` when it cannot be run or
/// exits non-zero. The seam every probe reads the world through.
pub trait CommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String>;
}

/// The production runner: spawns the command and keeps stdout on success only.
pub struct SystemCommandRunner;

impl CommandRunner for SystemCommandRunner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let output = Command::new(program).args(args).output().ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8(output.stdout).ok()
    }
}

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
pub fn parse_idle_nanoseconds(ioreg_output: &str) -> Option<&str> {
    ioreg_output
        .lines()
        .find(|line| line.contains(IOREG_IDLE_KEY))
        .and_then(|line| line.split_whitespace().last())
}

/// The process ids `pgrep` printed, one per line, discarding anything that is
/// not a plain decimal id.
pub fn parse_pids(pgrep_output: &str) -> Vec<String> {
    pgrep_output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// The focused pane id, from the multiplexer's JSON pane listing.
///
/// Parsed WITHOUT a JSON dependency: the listing is one object per pane on a
/// single line, so the pane carrying `"focused":true` is found by locating that
/// marker and reading the `pane_id` value nearest to it. A shape this module
/// does not recognise yields None, which fails OPEN (the card still fires).
pub fn parse_focused_pane(pane_list_json: &str) -> Option<String> {
    let focused_at = pane_list_json.find("\"focused\":true")?;
    let object_start = pane_list_json[..focused_at].rfind('{')?;
    let object = &pane_list_json[object_start..];
    let key = "\"pane_id\":\"";
    let value_start = object.find(key)? + key.len();
    let value_end = object[value_start..].find('"')?;
    Some(object[value_start..value_start + value_end].to_string())
}

/// The four probes: three read the machine through one command each, and the
/// marker reads the filesystem directly, because an mtime needs no subprocess.
///
/// One struct rather than four, because they share the runner and a caller
/// composes the traits it needs. SOLID: it depends on the runner abstraction,
/// never on `Command` directly, so the whole edge is substitutable.
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
        let modified = std::fs::metadata(&self.marker_path).ok()?.modified().ok()?;
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

impl<R: CommandRunner> crate::probes::FocusedPaneProbe for SystemProbes<R> {
    fn focused_pane(&self) -> Option<String> {
        // Resolved through PATH, unlike the system binaries above: the
        // multiplexer is not at a fixed location, and a context whose PATH does
        // not carry it reads as unknown, which fails OPEN into a card.
        let pane_list = self.runner.run("herdr", &["pane", "list"])?;
        parse_focused_pane(&pane_list)
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandRunner, parse_focused_pane, parse_idle_nanoseconds, parse_pids};
    use std::cell::RefCell;

    /// Records what it was asked to run and answers from a script, so a test
    /// pins both the parsing and the exact argv a probe uses.
    struct FakeRunner {
        answer: Option<String>,
        calls: RefCell<Vec<String>>,
    }

    impl FakeRunner {
        fn answering(answer: &str) -> Self {
            Self {
                answer: Some(answer.to_string()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                answer: None,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandRunner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Option<String> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));
            self.answer.clone()
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

    // --- parse_focused_pane -------------------------------------------------

    #[test]
    fn the_focused_pane_id_comes_from_the_object_carrying_the_focused_marker() {
        let json = r#"{"result":{"panes":[{"pane_id":"wW:p7","focused":false},{"pane_id":"wW:p21","focused":true}]}}"#;
        assert_eq!(parse_focused_pane(json), Some("wW:p21".to_string()));
    }

    #[test]
    fn a_listing_with_nothing_focused_reads_as_unknown_so_the_card_still_fires() {
        let json = r#"{"result":{"panes":[{"pane_id":"wW:p7","focused":false}]}}"#;
        assert_eq!(parse_focused_pane(json), None);
    }

    #[test]
    fn a_shape_this_parser_does_not_recognise_reads_as_unknown_rather_than_guessing() {
        assert_eq!(parse_focused_pane("not json at all"), None);
        assert_eq!(parse_focused_pane(""), None);
    }

    // --- the runner seam ----------------------------------------------------

    #[test]
    fn a_runner_that_cannot_run_the_command_yields_no_reading() {
        let runner = FakeRunner::failing();
        assert_eq!(runner.run("/usr/sbin/ioreg", &["-c", "IOHIDSystem"]), None);
    }

    #[test]
    fn a_probe_passes_its_arguments_through_verbatim_so_argv_is_pinned() {
        let runner = FakeRunner::answering("out");
        runner.run("/usr/sbin/ioreg", &["-c", "IOHIDSystem"]);
        assert_eq!(runner.calls.borrow()[0], "/usr/sbin/ioreg -c IOHIDSystem");
    }

    // --- the four probe implementations, the behavior R2a owes -------------

    use super::SystemProbes;
    use crate::probes::{FocusedPaneProbe, IdleProbe, MoshRateProbe, PhoneMarkerProbe};

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
    fn no_sessions_means_no_sample_rather_than_an_empty_one() {
        // An empty CSV would be judged INACTIVE either way, but skipping the
        // sampler is what keeps a full second of live counters off this path.
        let probes = probes_answering("");
        assert_eq!(probes.sample_csv(), None);
    }

    #[test]
    fn a_pane_listing_yields_the_focused_pane_through_the_probe() {
        let probes = probes_answering(
            "{\"result\":{\"panes\":[{\"pane_id\":\"wW:p21\",\"focused\":true}]}}",
        );
        assert_eq!(probes.focused_pane(), Some("wW:p21".to_string()));
    }

    #[test]
    fn a_multiplexer_that_cannot_be_reached_reports_unknown_and_the_card_fires() {
        assert_eq!(probes_failing().focused_pane(), None);
    }
}
