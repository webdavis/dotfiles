//! The uu binary: the composition root, and the only place with a main.
//!
//! Everything here is WIRING. The environment and the config are read once at
//! this edge, every decision is delegated to the library, and the three
//! process boundaries (the lane subjects, the pns engine, the hermes gateway)
//! each have exactly one implementation, right here.
//!
//! EXIT CODES SAY WHO FAILED. A lane that failed does not fail the run (0),
//! because the record is what reports it and the next attempt is a week away.
//! A config uu could not read, or a lane the operator asked for and did not
//! get, is uu failing (1). An argument uu does not serve is usage (2).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use unattended_upgrades::alert::{Alerter, alert_argv, alert_summary};
use unattended_upgrades::config::{
    Config, ConfigError, LANE_NAMES, LoadOutcome, Records, config_path, load_config,
};
use unattended_upgrades::lanes::{CommandRunner, LaneReport, enabled_lanes, run_lane};
use unattended_upgrades::record::{
    AGENT, Marker, gap_line, marker_contents, parse_marker, record_body, record_detail,
    record_state,
};
use unattended_upgrades::schedule::{DEFAULT_LABEL, render_plist};

fn main() {
    // Die on a closed pipe the way every other unix tool does. Rust ignores
    // SIGPIPE and turns the failed write into a panic instead, so `uu doctor |
    // head` would print a backtrace over the output it just produced.
    // SAFETY: restoring a signal's default disposition, before any thread or
    // handler of this program's own exists.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    std::process::exit(dispatch());
}

/// The whole CLI. A slice match rather than a chain, so an extra word is an
/// error instead of an argument nothing reads.
fn dispatch() -> i32 {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let words: Vec<&str> = argv.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["run"] => run_mode(None),
        ["run", lane] => run_mode(Some(lane)),
        ["doctor"] => doctor_mode(),
        ["schedule", "render"] => schedule_mode(),
        [] => usage("no command given"),
        [command, ..] => usage(&format!("unknown command `{command}`")),
    }
}

/// What uu serves, printed on stderr with a non-zero exit. Never a silent
/// fallthrough to help with exit 0.
fn usage(problem: &str) -> i32 {
    eprintln!(
        "uu: {problem}\n\
         usage:\n  \
           uu run [<lane>]     run every enabled lane, or just one\n  \
           uu doctor           what this config turns on, and what it cannot reach\n  \
           uu schedule render  the launchd job for the configured day and time\n\
         lanes: {}",
        LANE_NAMES.join(", ")
    );
    2
}

/// How long one signed record POST may take. Nobody is waiting on the answer,
/// so this only bounds how long the job lingers on a gateway that stopped
/// listening; it matches the deadline pns gives its own unwatched posts.
const RECORD_DEADLINE: Duration = Duration::from_secs(10);

// --- the run ---------------------------------------------------------------

fn run_mode(only: Option<&str>) -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };
    if let Some(lane) = only
        && !LANE_NAMES.contains(&lane)
    {
        return usage(&format!("`{lane}` is not a lane this build runs"));
    }

    let path = config_path(&home);
    let config = match loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!(
                "uu: no config at {}; nothing is enabled and nothing was updated",
                path.display()
            );
            return 0;
        }
        Err(code) => return code,
    };

    // The header is captured BEFORE the lanes run and before the marker is
    // rewritten: a gap sampled at delivery reports zero on every run.
    let now = now_epoch();
    let marker_path = marker_path(&home);
    let gap = gap_line(
        &read_marker(&marker_path),
        &marker_path.display().to_string(),
        now,
    );

    let runner = SystemRunner;
    let mut reports: Vec<LaneReport> = Vec::new();
    for name in enabled_lanes(&config) {
        if only.is_some_and(|wanted| wanted != name) {
            continue;
        }
        // CONTINUE ON FAILURE: nothing here inspects the report before moving
        // to the next lane.
        if let Some(report) = run_lane(name, &config, &runner) {
            reports.push(report);
        }
    }
    if let Some(lane) = only
        && reports.is_empty()
    {
        eprintln!(
            "uu: lane `{lane}` has no `[lanes.{lane}]` block in {}",
            path.display()
        );
        return 1;
    }

    let failures: usize = reports.iter().map(|report| report.failures).sum();
    let detail = record_detail(&host(), &iso(now), &gap, &reports);
    print!("{detail}");

    let engine = config.alerts.as_ref().map(|alerts| alerts.binary.clone());
    for report in reports.iter().filter(|report| report.failures > 0) {
        send_alert(engine.as_deref(), &report.name, &alert_summary(report));
    }

    if let Some(records) = config.records.as_ref() {
        deliver_record(records, records_body(failures, &detail), engine.as_deref());
    } else {
        println!("uu: no [records] block; this run was logged here and nowhere else");
    }

    // THE MARKER MOVES ONLY ON A CLEAN RUN, so the next entry's gap measures
    // the last time everything actually worked rather than the last time uu
    // woke up.
    if failures == 0 {
        write_marker(&marker_path, now);
    }
    0
}

fn records_body(failures: usize, detail: &str) -> String {
    record_body(record_state(failures), &host(), detail)
}

// --- the doctor ------------------------------------------------------------

fn doctor_mode() -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };
    let path = config_path(&home);
    println!("uu: config {}", path.display());
    let config = match loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("uu: no config file; every lane is off");
            Config::default()
        }
        Err(code) => return code,
    };

    let enabled = enabled_lanes(&config);
    for name in LANE_NAMES {
        let state = if enabled.contains(name) { "on" } else { "off" };
        println!("uu: lane {name}: {state}");
    }
    match config.records.as_ref() {
        // THE KEY IS NEVER PRINTED, only whether there is one.
        Some(records) => println!("uu: records: on, posting to {} (key set)", records.url),
        None => println!("uu: records: off, no [records] block"),
    }
    match config.alerts.as_ref() {
        Some(alerts) => {
            let reachable = match resolve(&alerts.binary) {
                Some(found) => format!("found at {}", found.display()),
                None => "NOT FOUND; failures will be logged and nothing else".to_string(),
            };
            println!("uu: alerts: on via `{}`, {reachable}", alerts.binary);
        }
        None => println!("uu: alerts: off, no [alerts] block"),
    }
    let schedule = config.schedule;
    println!(
        "uu: schedule: weekday {} at {:02}:{:02} (this feeds `uu schedule render` only)",
        schedule.weekday, schedule.hour, schedule.minute
    );
    let marker_path = marker_path(&home);
    println!(
        "uu: {}",
        gap_line(
            &read_marker(&marker_path),
            &marker_path.display().to_string(),
            now_epoch()
        )
    );
    0
}

// --- the schedule ----------------------------------------------------------

fn schedule_mode() -> i32 {
    let Some(home) = home() else {
        eprintln!("uu: HOME is not set, so there is no config to read");
        return 1;
    };
    let config = match loaded(&config_path(&home)) {
        Ok(Some(config)) => config,
        Ok(None) => Config::default(),
        Err(code) => return code,
    };
    print!(
        "{}",
        render_plist(
            DEFAULT_LABEL,
            &installed_binary(&home).display().to_string(),
            &log_path(&home).display().to_string(),
            config.schedule,
        )
    );
    0
}

// --- the edges -------------------------------------------------------------

/// The config, or `None` for a machine that has not written one. A refusal is
/// printed here and returned as an exit code, because every mode answers it
/// the same way: loudly, and without guessing.
fn loaded(path: &Path) -> Result<Option<Config>, i32> {
    match load_config(path) {
        Ok(LoadOutcome::Loaded(config)) => Ok(Some(config)),
        Ok(LoadOutcome::Missing) => Ok(None),
        Err(error) => {
            let what = match error {
                ConfigError::Malformed(_) => "is not valid TOML",
                ConfigError::Invalid(_) => "is not a config uu can use",
                ConfigError::Unreadable(_) => "could not be read",
            };
            eprintln!("uu: {} {what}: {}", path.display(), error.detail());
            Err(1)
        }
    }
}

fn home() -> Option<String> {
    std::env::var("HOME").ok().filter(|home| !home.is_empty())
}

fn marker_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/state/uu/last-success")
}

fn installed_binary(home: &str) -> PathBuf {
    Path::new(home).join(".local/libexec/uu/uu")
}

fn log_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/log/uu/uu.log")
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

/// ISO 8601 UTC, computed from the same epoch the gap is, so the two figures
/// in one entry can never be sampled at different instants.
fn iso(epoch: i64) -> String {
    let mut when = std::mem::MaybeUninit::<libc::tm>::uninit();
    let seconds = epoch as libc::time_t;
    // SAFETY: gmtime_r writes into the caller's own tm and returns null only
    // when it wrote nothing, which is the branch below.
    let filled = unsafe { libc::gmtime_r(&seconds, when.as_mut_ptr()) };
    if filled.is_null() {
        return format!("epoch {epoch}");
    }
    let when = unsafe { when.assume_init() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        when.tm_year + 1900,
        when.tm_mon + 1,
        when.tm_mday,
        when.tm_hour,
        when.tm_min,
        when.tm_sec
    )
}

/// The machine this record is about. The channel aggregates unattended jobs
/// from more than one host, so an entry that does not name its host is not
/// investigable.
fn host() -> String {
    let mut name = [0_i8; 256];
    // SAFETY: the buffer and its length are this frame's own, and the result
    // is only read after a success, truncated at the first NUL.
    let read = unsafe { libc::gethostname(name.as_mut_ptr(), name.len()) };
    if read != 0 {
        return "unknown-host".to_string();
    }
    let bytes: Vec<u8> = name
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| *byte as u8)
        .collect();
    let full = String::from_utf8_lossy(&bytes).to_string();
    // The short name, matching `hostname -s`: a bonjour suffix says nothing
    // an entry needs.
    let short = full.split('.').next().unwrap_or_default().to_string();
    if short.is_empty() {
        "unknown-host".to_string()
    } else {
        short
    }
}

fn read_marker(path: &Path) -> Marker {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_marker(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Marker::NeverRecorded,
        Err(_) => Marker::Unreadable,
    }
}

/// Best effort, and never silent: a job must not fail because it could not
/// write its own bookkeeping, but a failure to write would have the next entry
/// measure its gap from a run that did not happen.
fn write_marker(path: &Path, epoch: i64) {
    let written = path
        .parent()
        .map(std::fs::create_dir_all)
        .transpose()
        .and_then(|_| std::fs::write(path, marker_contents(epoch, &iso(epoch))));
    if let Err(error) = written {
        eprintln!(
            "uu: could not record the successful-run timestamp at {}: {error}; the next entry \
             will report a stale or absent gap",
            path.display()
        );
    }
}

/// Where a command name resolves on this PATH, or `None`. An absolute or
/// relative path is answered from the filesystem directly, the way a shell
/// does.
fn resolve(command: &str) -> Option<PathBuf> {
    let runnable = |path: &Path| path.is_file();
    if command.contains('/') {
        let path = PathBuf::from(command);
        return runnable(&path).then_some(path);
    }
    std::env::var_os("PATH")?
        .to_string_lossy()
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(command))
        .find(|candidate| runnable(candidate))
}

/// The lane subjects. No deadline, matching the shell job this replaces: a
/// plugin install has no honest upper bound and launchd is what notices a job
/// that never ends.
struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str]) -> Result<String, String> {
        match Command::new(program).args(args).output() {
            Err(error) => Err(format!("could not run {program}: {error}")),
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            }
            Ok(output) => Err(match output.status.code() {
                Some(code) => format!("exit {code}"),
                None => "killed by a signal".to_string(),
            }),
        }
    }
}

/// The pns engine, as a client: flags in, nothing read back.
struct PnsAlerter;

impl Alerter for PnsAlerter {
    fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
        match Command::new(binary).args(args).status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!("`{binary}` answered {status}")),
            Err(error) => Err(format!("`{binary}` could not be run: {error}")),
        }
    }
}

/// One alert, FAIL OPEN at every rung: no `[alerts]` block, an engine that is
/// not there, and an engine that refused are each stated here and none of them
/// ends the run.
fn send_alert(engine: Option<&str>, lane: &str, summary: &str) {
    let Some(binary) = engine else {
        println!("uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else");
        return;
    };
    let argv = alert_argv(&host(), lane, summary);
    if let Err(why) = PnsAlerter.alert(binary, &argv) {
        println!("uu: the alert for `{lane}` was NOT delivered ({why}); it is logged here instead");
    }
}

/// The record, posted in process. FAIL LOUD: a refused delivery is printed AND
/// alerted, because a silent record channel is indistinguishable from a
/// machine whose jobs stopped running, which is the one failure the record
/// cannot report about itself.
fn deliver_record(records: &Records, body: String, engine: Option<&str>) {
    use pns::channels::hermes::{SignedPost, UreqSignedPost, delivered, outcome_line, sign};

    let Some(signature) = sign(&records.key, &body) else {
        println!("uu: the [records] key is empty, so nothing could be signed or posted");
        return;
    };
    let outcome = UreqSignedPost.post(&records.url, &body, &signature, Some(RECORD_DEADLINE));
    println!("uu: {}", outcome_line(outcome));
    if !delivered(outcome) {
        send_alert(
            engine,
            AGENT,
            &format!(
                "the weekly record could NOT be delivered to {} ({}); until this is fixed that \
                 channel is silent for a reason that has nothing to do with the jobs it reports on",
                records.url,
                outcome_line(outcome)
            ),
        );
    }
}
