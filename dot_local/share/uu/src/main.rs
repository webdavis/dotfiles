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

use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use pns::channels::hermes::{SignedPost, UreqSignedPost, delivered, outcome_line, sign};
use unattended_upgrades::alert::{Alerter, alert_argv, alert_summary};
use unattended_upgrades::config::{
    Config, ConfigError, LANE_TYPES, LaneKind, LoadOutcome, Records, config_path, load_config,
};
use unattended_upgrades::deadline::lane_budget;
use unattended_upgrades::lanes::{LaneReport, run_lane};
use unattended_upgrades::record::{
    AGENT, Marker, RunFacts, STALE_AFTER_RUNS, gap_line, marker_contents, next_streak,
    parse_marker, record_body, record_detail, record_state,
};
use unattended_upgrades::schedule::{DEFAULT_LABEL, render_plist};

mod runner;
mod watchdog;
use runner::SystemRunner;

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
         lane types: {}",
        LANE_TYPES.join(", ")
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

    let path = config_path(&home);
    let config = match loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            // A bare run on a configless machine is clean by design. A lane
            // asked for BY NAME is a request, and one no file declares did
            // not run, so it is refused the way an undeclared name is below.
            if let Some(lane) = only {
                eprintln!(
                    "uu: no config at {}, so no lane `{lane}` is declared",
                    path.display()
                );
                return 1;
            }
            println!(
                "uu: no config at {}; nothing is enabled and nothing was updated",
                path.display()
            );
            return 0;
        }
        Err(code) => return code,
    };

    // ONE RUN AT A TIME. Everything below reads, then writes, the marker and
    // every lane's own streak file with no other guard: two overlapping runs
    // can both read the same streak count and both write the same next
    // value, which is the one mechanism whose entire job is noticing a lane
    // gone quiet, so a delayed or duplicated staleness alert is the exact
    // failure this verdict exists to prevent. Non-blocking, matching the two
    // weekly jobs this ported from (`/usr/bin/lockf -s -t 0`, kernel-backed
    // and released automatically on exit or a crash): a second run says so
    // and exits rather than pretending it ran.
    let _lock = match acquire_run_lock(&home) {
        Ok(lock) => lock,
        Err(LockFailure::Contended(why)) => {
            eprintln!("uu: {why}; not running, to avoid racing the run that already holds it");
            return 1;
        }
        Err(LockFailure::Unavailable(why)) => {
            eprintln!("uu: {why}; not running");
            return 1;
        }
    };

    // A LANE NO LONGER DECLARED IS PRUNED HERE, under the same lock: its
    // directory (and whatever streak it held) would otherwise leak forever,
    // and a NEW lane that reuses the old name would inherit that streak and
    // could alert on its very first miss.
    prune_removed_lane_state(&home, &config);

    // The header is captured BEFORE the lanes run and before the marker is
    // rewritten: a gap sampled at delivery reports zero on every run. A clock
    // that cannot be read renders as the epoch itself, which is a date no
    // reader mistakes for a real one.
    let started = now_epoch().unwrap_or(0);
    let started_iso = iso(started);
    let marker_path = marker_path(&home);
    let marker = read_marker(&marker_path);
    let gap = gap_line(&marker, &marker_path.display().to_string(), started);
    let host_name = host();
    let facts = RunFacts {
        host: &host_name,
        started_epoch: started,
        started_iso: &started_iso,
        marker: &marker,
    };

    let mut reports: Vec<LaneReport> = Vec::new();
    // THE RUN'S OWN CLOCK, started under the lock this loop holds. Lanes run in
    // sequence and nothing caps how many a config declares, so each lane's
    // budget is capped by what is left of the run's (`lane_budget`).
    let run_started = Instant::now();
    // IN NAME ORDER, never the file's: `lanes` is a `BTreeMap`, and a run
    // whose sequence changes when a block moves is a run nobody can reason
    // about.
    for (name, lane) in &config.lanes {
        if only.is_some_and(|wanted| wanted != name) {
            continue;
        }
        // ONE RUNNER PER LANE, holding that lane's own budget: it is the whole
        // lane's, and its clock starts here.
        let runner = SystemRunner::for_lane(
            name,
            lane_budget(lane.deadline, run_started.elapsed()),
            lane.deadline,
        );
        // CONTINUE ON FAILURE: nothing here inspects the report before moving
        // to the next lane.
        if let Some(report) = run_lane(name, &config, &facts, &runner) {
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
    let deferred: usize = reports.iter().filter(|report| report.deferred).count();
    let detail = record_detail(&host_name, &started_iso, &gap, &reports);
    print!("{detail}");

    let engine = config.alerts.as_ref().map(|alerts| alerts.binary.clone());
    for report in reports.iter().filter(|report| report.failures > 0) {
        send_alert(
            &PnsAlerter,
            engine.as_deref(),
            &report.name,
            &alert_summary(report),
        );
    }

    // THE STALENESS BOUND: a lane deferring or failing every week is silent
    // by design (a deferral never alerts on its own, and even a failure's
    // alert says nothing about HOW LONG this has been going on), so nothing
    // else says a lane has gone quiet for good. Tracked PER LANE, across
    // runs, independent of whatever else this run's own verdict says.
    for report in &reports {
        let succeeded = !report.deferred && report.failures == 0;
        let path = streak_path(&home, &report.name);
        // A STREAK THIS RUN COULD NOT TRUST is never read as zero: zero would
        // silently forgive whatever history the file held, which is the
        // opposite of what a mechanism built to notice a lane going quiet
        // may ever do. Treated as one short of the threshold instead, so a
        // non-success run still gets its chance to trip rather than starting
        // a fresh count nobody asked for.
        let previous = match read_streak(&path) {
            Streak::Absent => 0,
            Streak::Value(value) => value,
            Streak::Unreadable(why) => {
                send_alert(
                    &PnsAlerter,
                    engine.as_deref(),
                    &report.name,
                    &format!(
                        "this lane's non-success streak at {} could not be trusted ({why}); \
                         treating it as already close to stale rather than silently starting \
                         over",
                        path.display()
                    ),
                );
                STALE_AFTER_RUNS - 1
            }
        };
        let (next, tripped) = next_streak(previous, succeeded);
        // AN UNDELIVERED TRIP IS RETRIED, NEVER LOST. This alert fires once
        // per streak, so an engine that was down for the one run that trips
        // would otherwise leave a deferring lane silent for good: a deferral
        // raises nothing else, and the streak only climbs from here. Holding
        // the count one short of the threshold makes the next run trip again.
        // The count is read by nothing but this trip, so a run spent short of
        // its true value costs no reader anything.
        let recorded = if tripped
            && !send_alert(
                &PnsAlerter,
                engine.as_deref(),
                &report.name,
                &format!(
                    "no successful run in {STALE_AFTER_RUNS} consecutive attempt(s); the last \
                     one {}",
                    if report.deferred {
                        "deferred"
                    } else {
                        "failed"
                    }
                ),
            ) {
            STALE_AFTER_RUNS - 1
        } else {
            next
        };
        // A WRITE FAILURE IS LOUD, never just an eprintln nobody reads from a
        // headless launchd job: this file IS the mechanism, so losing it
        // silently would be exactly the fail-open this whole capability
        // exists to refuse.
        if let Err(why) = write_streak(&path, recorded) {
            eprintln!(
                "uu: could not record lane `{}`'s non-success streak at {}: {why}",
                report.name,
                path.display()
            );
            send_alert(
                &PnsAlerter,
                engine.as_deref(),
                &report.name,
                &format!(
                    "this lane's non-success streak at {} could not be recorded ({why}); \
                     staleness tracking for it is unreliable until this is fixed",
                    path.display()
                ),
            );
        }
    }

    // A RECORD THE GATEWAY NEVER RECEIVED IS A FAILED RUN, even when every
    // lane passed. The entry is the whole point of the week's work, and the
    // marker is what the NEXT entry measures its gap from: stamping a success
    // nothing recorded would have that entry claim a gap from a week no one
    // can read. With no `[records]` block nothing was owed, so nothing is
    // lost.
    let record_lost = match config.records.as_ref() {
        Some(records) => !deliver_record(
            &UreqSignedPost,
            &PnsAlerter,
            records,
            records_body(failures, deferred, &detail),
            engine.as_deref(),
        ),
        None => {
            println!("uu: no [records] block; this run was logged here and nowhere else");
            false
        }
    };

    // THE MARKER MOVES ONLY ON A CLEAN RUN, so the next entry's gap measures
    // the last time everything actually worked rather than the last time uu
    // woke up.
    //
    // A DEFERRAL IS NOT A CLEAN RUN EITHER. A deferred lane did no work, so it
    // must not count as the run that advances the marker: the marker means
    // the last time everything actually ran and succeeded, and letting a
    // deferral through would have a lane that never runs read as healthy
    // forever, which is exactly the failure mode this verdict exists to make
    // visible instead.
    //
    // AND IT STAMPS THE MOMENT THE RUN FINISHED, read here rather than reused
    // from the header. Lanes have no upper bound, so the header's instant can
    // be an hour old by now, and every following gap would carry that hour on
    // top of its own. A clock that will not answer at this instant leaves the
    // marker alone: an unmoved marker overstates the next gap, while a
    // guessed timestamp understates it silently.
    if failures == 0 && deferred == 0 && !record_lost {
        match now_epoch() {
            Some(finished) => write_marker(&marker_path, finished),
            None => eprintln!(
                "uu: this clock could not be read at the end of the run, so the successful-run \
                 timestamp at {} was left as it was; the next entry measures its gap from the \
                 run before this one",
                marker_path.display()
            ),
        }
    }
    0
}

fn records_body(failures: usize, deferred: usize, detail: &str) -> String {
    record_body(record_state(failures, deferred), &host(), detail)
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

    if config.lanes.is_empty() {
        println!("uu: lanes: none declared");
    } else {
        for (name, lane) in &config.lanes {
            println!("uu: lane {name}: on ({})", lane.kind.type_name());
            if let LaneKind::Command(command) = &lane.kind {
                let program = &command.run[0];
                // A SLASH-RELATIVE PROGRAM (`./updater`) is answered from
                // DOCTOR'S OWN cwd, wherever the operator happens to be
                // standing; the weekly launchd job starts at `/`, so `found`
                // or `NOT FOUND` here says nothing about what that run will
                // see. An absolute path or a bare name on PATH resolves the
                // same way in both places, so only this case gets its own
                // line instead of a resolution.
                if program.contains('/') && !program.starts_with('/') {
                    println!(
                        "uu: lane {name}: program `{program}`, RELATIVE PATH; the weekly run \
                         starts in /, so this resolves differently there"
                    );
                } else {
                    let reachable = match resolve(program) {
                        Some(found) => format!("found at {}", found.display()),
                        None => "NOT FOUND; every scheduled run of this lane will fail, and it \
                                 alerts only when [alerts] is configured"
                            .to_string(),
                    };
                    println!(
                        "uu: lane {name}: program `{program}`, {reachable} (doctor resolves on \
                         this shell's PATH; the weekly run uses the plist's own PATH, which can \
                         differ)"
                    );
                }
            }
        }
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
            now_epoch().unwrap_or(0)
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
    print!("{}", render_plist(DEFAULT_LABEL, &home, config.schedule));
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

/// Where a lane's non-success streak lives: one small file per lane, named for
/// the lane itself so two lanes never share bookkeeping.
fn streak_path(home: &str, lane: &str) -> PathBuf {
    Path::new(home)
        .join(".local/state/uu/lanes")
        .join(lane)
        .join("streak")
}

/// Where the whole run's own lock lives, held for as long as `run_mode` is
/// on the stack.
fn run_lock_path(home: &str) -> PathBuf {
    Path::new(home).join(".local/state/uu/run.lock")
}

/// The run-wide lock. Held only by virtue of the open file descriptor: the
/// kernel drops the `flock` the moment it closes, on a normal return or a
/// crash alike, so there is no stale-lock file to clean up by hand.
struct RunLock(#[allow(dead_code)] std::fs::File);

/// Why `acquire_run_lock` could not hand back a lock: the ONE arm that is
/// genuine contention, and everything else. The call site says something
/// different for each, because "to avoid racing the run that already holds
/// it" is only true for `Contended`: a directory that could not be created
/// or a lock file that could not even be opened is an environment problem
/// with its own real cause, and this is the one place the operator hears
/// about it, so blaming a race that never happened would send them chasing
/// the wrong thing.
enum LockFailure {
    /// `flock` itself refused: another run genuinely holds the lock right
    /// now.
    Contended(String),
    /// The lock file, or the directory it lives in, could not even be
    /// opened.
    Unavailable(String),
}

/// Take the run lock, or say why not. NON-BLOCKING (`LOCK_NB`): a second run
/// finding this one still going must say so and exit, never wait its turn
/// and then run stale.
fn acquire_run_lock(home: &str) -> Result<RunLock, LockFailure> {
    let path = run_lock_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            LockFailure::Unavailable(format!("could not create {}: {error}", parent.display()))
        })?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            LockFailure::Unavailable(format!("could not open {}: {error}", path.display()))
        })?;
    // SAFETY: `file`'s descriptor is open and owned by this frame for the
    // whole call; `flock`'s only effect is the kernel's own lock table.
    let refused = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0;
    if refused {
        return Err(LockFailure::Contended(format!(
            "another run already holds {}",
            path.display()
        )));
    }
    Ok(RunLock(file))
}

/// The lane names this config no longer declares are dropped here, under the
/// run lock: a directory left behind by a removed or renamed lane would leak
/// forever otherwise, and a NEW lane reusing the old name would inherit its
/// streak and could alert on its very first miss. Best effort and silent on
/// its own failure, matching every other piece of this bookkeeping: a stale
/// directory that resists cleanup costs nothing but a few bytes, never a
/// wrong verdict.
fn prune_removed_lane_state(home: &str, config: &Config) {
    let lanes_dir = Path::new(home).join(".local/state/uu/lanes");
    let Ok(entries) = std::fs::read_dir(&lanes_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        if !config.lanes.contains_key(&name) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// What reading a lane's streak file found.
///
/// `Absent` COVERS ONLY `NotFound`: that is the one case that legitimately
/// means a fresh lane, or one that has never had a non-success run. Anything
/// else the file could say (unreadable, a directory sitting where the file
/// belongs, content that is not a plain count) is `Unreadable`, never a
/// silent zero: zero would forgive whatever streak the file actually held,
/// which is the fail-open this whole capability exists to refuse.
#[derive(Debug, PartialEq, Eq)]
enum Streak {
    Absent,
    Value(u32),
    Unreadable(String),
}

fn read_streak(path: &Path) -> Streak {
    match std::fs::read_to_string(path) {
        Ok(text) => match text.trim().parse() {
            Ok(value) => Streak::Value(value),
            Err(_) => Streak::Unreadable(format!("{:?} is not a plain count", text.trim())),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Streak::Absent,
        Err(error) => Streak::Unreadable(error.to_string()),
    }
}

/// Publish a lane's streak, ATOMICALLY: a sibling temp file, written in full
/// and then renamed over the target. `rename` only needs write permission on
/// the DIRECTORY, never on the file it replaces, so a streak file an earlier
/// run left read-only no longer blocks every write after it the way
/// truncating in place did; only a directory this run cannot write to still
/// fails, and that failure is returned rather than swallowed, so the caller
/// can make it loud instead of silent.
fn write_streak(path: &Path, value: u32) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "the streak path has no parent directory".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    // NAMED FOR THE TARGET, not only the process: every lane's own streak
    // lives in its own directory in production, but the unit tests below
    // exercise several DIFFERENT target files under the same shared temp
    // parent, in the same process, in parallel. A temp name keyed on the
    // process id alone collided across those targets; keying it on the
    // target's own file name as well makes two DIFFERENT targets in the SAME
    // directory use different temp files even when the process id matches.
    let target_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("streak");
    let temp = parent.join(format!(".{target_name}-{}.tmp", std::process::id()));
    std::fs::write(&temp, format!("{value}\n")).map_err(|error| error.to_string())?;
    std::fs::rename(&temp, path).map_err(|error| error.to_string())
}

/// This instant in epoch seconds, or `None` for a clock set before 1970. The
/// caller decides what an unreadable clock means: the header prints it, the
/// marker refuses to move on it.
fn now_epoch() -> Option<i64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .map(|since| since.as_secs() as i64)
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

/// The marker, with an ABSENT path told from a BROKEN LINK. Both read
/// NotFound, and they are opposite states: nothing there is a machine that has
/// never finished a run, while a link whose target went away is bookkeeping
/// that stopped resolving and has to be said out loud.
fn read_marker(path: &Path) -> Marker {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_marker(&text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && std::fs::symlink_metadata(path).is_err() =>
        {
            Marker::NeverRecorded
        }
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
///
/// ANSWERS WHETHER THE ALERT IS OWED ANY LONGER, which is not the same as
/// whether an engine ran. With no `[alerts]` block nothing was owed and the
/// log line IS the delivery, so that is `true`; only a configured engine that
/// refused leaves something still to say. The per-run failure alert ignores
/// this, because it fires again next run either way; the staleness alert
/// fires once per streak and has to know.
fn send_alert(alerter: &dyn Alerter, engine: Option<&str>, lane: &str, summary: &str) -> bool {
    let Some(binary) = engine else {
        println!("uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else");
        return true;
    };
    let argv = alert_argv(&host(), lane, summary);
    if let Err(why) = alerter.alert(binary, &argv) {
        println!("uu: the alert for `{lane}` was NOT delivered ({why}); it is logged here instead");
        return false;
    }
    true
}

/// The record, posted in process, ANSWERING WHETHER IT LANDED. The caller
/// needs that verdict: an entry the gateway never received is a failed run,
/// whatever the lanes did.
///
/// FAIL LOUD: a refused delivery is printed AND alerted, because a silent
/// record channel is indistinguishable from a machine whose jobs stopped
/// running, which is the one failure the record cannot report about itself.
///
/// BOTH PROCESS BOUNDARIES ARRIVE AS TRAITS, never the concrete client: a
/// refused delivery can only be exercised through a real socket failure
/// otherwise, and the alert it fires is invisible to anything but a real
/// engine. `run_mode` is the only caller and hands in the production pair.
fn deliver_record(
    post: &dyn SignedPost,
    alerter: &dyn Alerter,
    records: &Records,
    body: String,
    engine: Option<&str>,
) -> bool {
    let Some(signature) = sign(&records.key, &body) else {
        println!("uu: the [records] key is empty, so nothing could be signed or posted");
        return false;
    };
    let outcome = post.post(&records.url, &body, &signature, Some(RECORD_DEADLINE));
    println!("uu: {}", outcome_line(outcome));
    if delivered(outcome) {
        return true;
    }
    send_alert(
        alerter,
        engine,
        AGENT,
        &format!(
            "the weekly record could NOT be delivered to {} ({}); until this is fixed that \
             channel is silent for a reason that has nothing to do with the jobs it reports on",
            records.url,
            outcome_line(outcome)
        ),
    );
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A path of this test's own under the temp directory, with nothing at it.
    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("uu-main-{name}-{}", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn a_path_with_no_marker_at_it_is_a_machine_that_never_recorded_a_run() {
        assert_eq!(read_marker(&scratch("absent")), Marker::NeverRecorded);
    }

    // --- the staleness streak's file I/O ----------------------------------

    #[test]
    fn a_path_with_no_streak_at_it_is_absent_which_is_the_only_case_that_reads_as_zero() {
        assert_eq!(read_streak(&scratch("streak-absent")), Streak::Absent);
    }

    #[test]
    fn a_streak_file_that_does_not_parse_as_a_count_is_unreadable_not_a_silent_zero() {
        // BEFORE THIS FIX this read as zero, which silently forgives whatever
        // streak a half-written or corrupted file actually held: a lane one
        // run short of tripping would quietly restart its count from
        // scratch instead of the operator ever hearing about it.
        let path = scratch("streak-garbage");
        std::fs::write(&path, "not-a-number\n").expect("the file");
        let read = read_streak(&path);
        std::fs::remove_file(&path).ok();
        assert!(matches!(read, Streak::Unreadable(_)), "{read:?}");
    }

    #[test]
    fn a_streak_file_this_process_cannot_read_is_unreadable_not_absent() {
        // Distinct from `Absent`: the file IS there, so a lane whose history
        // this run cannot see must not be told it has none.
        let path = scratch("streak-unreadable-mode");
        std::fs::write(&path, "2\n").expect("the file");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("mode");
        let read = read_streak(&path);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).ok();
        std::fs::remove_file(&path).ok();
        assert!(matches!(read, Streak::Unreadable(_)), "{read:?}");
    }

    #[test]
    fn a_written_streak_is_the_streak_read_back() {
        let path = scratch("streak-roundtrip");
        write_streak(&path, 2).expect("the write");
        let read = read_streak(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(read, Streak::Value(2));
    }

    #[test]
    fn writing_a_streak_creates_its_parent_directory() {
        let path = std::env::temp_dir().join(format!(
            "uu-main-streak-parent-{}/lanes/mine/streak",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap());
        write_streak(&path, 1).expect("the write");
        let read = read_streak(&path);
        std::fs::remove_dir_all(path.parent().unwrap().parent().unwrap()).ok();
        assert_eq!(read, Streak::Value(1));
    }

    #[test]
    fn a_streak_file_made_read_only_is_still_overwritten_by_the_next_run() {
        // ROW 2, DIRECTION B reproduced by hand before this fix: a streak
        // file made read-only after being written stayed stuck at its old
        // value forever, because a plain `fs::write` truncates the EXISTING
        // file in place and needs write permission on it. Publishing through
        // a rename needs write permission on the DIRECTORY only, so the same
        // read-only file no longer blocks the write at all.
        let path = scratch("streak-readonly-file");
        write_streak(&path, 2).expect("the first write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o444)).expect("mode");
        let result = write_streak(&path, 3);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).ok();
        result.expect("a rename over a read-only file must still succeed");
        let read = read_streak(&path);
        std::fs::remove_file(&path).ok();
        assert_eq!(read, Streak::Value(3));
    }

    #[test]
    fn a_streak_write_whose_directory_cannot_be_created_reports_why_rather_than_staying_silent() {
        // ROW 2, DIRECTION A reproduced by hand before this fix: a plain FILE
        // sitting where the lane's directory belongs makes `create_dir_all`
        // fail on every run, and the old `write_streak` only printed to
        // stderr and moved on, so a lane stuck this way never once reached
        // the staleness threshold: the count could never actually persist.
        let blocker = scratch("streak-blocked-parent");
        std::fs::write(&blocker, "").expect("a plain file occupying the would-be directory");
        let path = blocker.join("streak");
        let error = write_streak(&path, 1).expect_err("a file cannot become a directory");
        std::fs::remove_file(&blocker).ok();
        assert!(!error.is_empty(), "{error:?}");
    }

    #[test]
    fn a_dangling_marker_symlink_is_unreadable_rather_than_never_recorded() {
        // A broken link reads NotFound exactly like an absent path, and the
        // two are opposite states: nothing recorded yet is a fresh machine,
        // while a link whose target went away is bookkeeping that STOPPED
        // resolving. Read as "never recorded" it reports a fresh machine
        // forever and no gap is ever measured again.
        let link = scratch("marker-dangling");
        std::os::unix::fs::symlink("uu-absent-target", &link).expect("the link");
        let marker = read_marker(&link);
        std::fs::remove_file(&link).ok();
        assert_eq!(marker, Marker::Unreadable);
    }

    // --- the posted record body -------------------------------------------

    #[test]
    fn a_deferred_only_run_posts_a_body_stated_deferred_not_completed() {
        // record_state itself is pinned directly in record.rs; this instead
        // guards the CALL SITE here in `records_body`, where a mutant
        // passing `record_state(failures, 0)` would post every deferred-only
        // run as "completed" while leaving every `record_state` unit test
        // green.
        let body = records_body(0, 1, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "deferred");
    }

    #[test]
    fn a_mixed_run_posts_a_body_stated_failed_not_deferred() {
        let body = records_body(1, 1, "detail");
        let parsed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed["state"], "failed");
    }

    // --- the record and alert seams --------------------------------------

    use pns::channels::hermes::PostOutcome;
    use std::cell::RefCell;

    /// A `SignedPost` stub that always answers the same fixed outcome. It
    /// never touches a socket, so both directions below run in well under a
    /// second and neither depends on a real gateway being up or down.
    struct AnswerWith(PostOutcome);

    impl SignedPost for AnswerWith {
        fn post(
            &self,
            _url: &str,
            _body: &str,
            _signature_hex: &str,
            _deadline: Option<Duration>,
        ) -> PostOutcome {
            self.0
        }
    }

    /// An `Alerter` that records every call instead of spawning anything, so
    /// a test can assert whether the alert path fired at all.
    #[derive(Default)]
    struct SpyAlerter {
        calls: RefCell<Vec<(String, Vec<String>)>>,
    }

    impl Alerter for SpyAlerter {
        fn alert(&self, binary: &str, args: &[String]) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push((binary.to_string(), args.to_vec()));
            Ok(())
        }
    }

    fn stub_records() -> Records {
        Records {
            url: "http://127.0.0.1:0/wherever".to_string(),
            key: "k".to_string(),
        }
    }

    #[test]
    fn a_refused_post_reports_failure_and_alerts_through_the_given_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::NoResponse),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(!delivered);
        let calls = spy.calls.borrow();
        assert_eq!(calls.len(), 1, "{calls:?}");
        assert_eq!(calls[0].0, "engine", "{calls:?}");
        assert!(
            calls[0]
                .1
                .iter()
                .any(|arg| arg.contains(&stub_records().url)),
            "{calls:?}"
        );
    }

    #[test]
    fn a_delivered_post_reports_success_and_never_touches_the_alerter() {
        let spy = SpyAlerter::default();
        let delivered = deliver_record(
            &AnswerWith(PostOutcome::Status(200)),
            &spy,
            &stub_records(),
            "body".to_string(),
            Some("engine"),
        );
        assert!(delivered);
        assert!(spy.calls.borrow().is_empty(), "{:?}", spy.calls.borrow());
    }
}
