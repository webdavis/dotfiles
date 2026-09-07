use crate::*;

// --- the daemon -------------------------------------------------------------

/// `pns daemon <verb>`: the clock, and the two typed commands that feed it.
///
/// A BARE `pns daemon` IS A REFUSAL, per the house rule that an unknown
/// argument never falls through to help with exit 0: a verb this does not serve
/// is a command the operator believes ran.
pub(crate) fn daemon_mode(verb: &str) -> i32 {
    match verb {
        "run" => daemon_run(),
        "schedule" => daemon_schedule(),
        "cancel" => daemon_cancel(),
        _ => {
            eprintln!("{DAEMON_USAGE}");
            2
        }
    }
}

pub(crate) const DAEMON_USAGE: &str = "pns: usage: pns daemon run | \
pns daemon schedule --id <id> [--in <secs>] [--every <secs>] [--until +<secs>|<epoch>] \
[--unless-marker <name>] -- <event args> | \
pns daemon cancel --id <id>";
/// `pns daemon schedule`: one registration, typed.
///
/// FOR DRILLS AND FOR TESTS. The library function beneath it is what a rider
/// will call, in-process, so nothing ever spawns a process to talk to the
/// daemon.
fn daemon_schedule() -> i32 {
    let argv: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    let Some(request) = parse_schedule(&argv) else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    let Some(now) = now_secs() else {
        eprintln!("pns daemon: this machine has no clock to schedule against");
        return 1;
    };
    let due = now.saturating_add(request.in_secs);
    let job = pns::daemon::Job {
        id: request.id,
        due,
        until: match request.until {
            Some(Until::Epoch(epoch)) => epoch,
            Some(Until::FromNow(seconds)) => now.saturating_add(seconds),
            // A LEASE IS NEVER ABSENT, only unstated: a job with no expiry is
            // the parked job the whole design refuses, so an unstated one gets
            // a small slack past its due second.
            None => due.saturating_add(DEFAULT_LEASE_SLACK_SECS),
        },
        every: request.every,
        unless_marker: request.marker,
        args: request.args,
    };
    match pns::daemon::schedule(&state_dir(), &job, now) {
        Ok(()) => 0,
        Err(refusal) => {
            eprintln!("pns daemon: {refusal}");
            1
        }
    }
}

/// How long past its due second an unstated lease runs. A minute: long enough
/// that a busy tick or a slow boot still delivers, short enough that a machine
/// asleep through the moment wakes to a job whose point has passed.
const DEFAULT_LEASE_SLACK_SECS: u64 = 60;

/// `--until` in its two spellings.
enum Until {
    Epoch(u64),
    FromNow(u64),
}

/// Everything `schedule` was asked for, before a clock is read.
struct ScheduleRequest {
    id: String,
    in_secs: u64,
    every: Option<u64>,
    until: Option<Until>,
    marker: Option<String>,
    args: Vec<String>,
}

/// The typed request, or None for anything this will not run.
///
/// UNKNOWN IS AN ERROR, never a silent skip: `pns`'s own event parser is
/// lenient because it sits on a notification path that must not fail, and this
/// one sits in front of an operator who typed a command and will believe it
/// did what they wrote.
fn parse_schedule(argv: &[String]) -> Option<ScheduleRequest> {
    let mut id = None;
    let mut in_secs = 0;
    let mut every = None;
    let mut until = None;
    let mut marker = None;
    let mut args = Vec::new();
    let mut words = argv.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            // Everything past the separator is the event, untouched.
            "--" => {
                args = words.cloned().collect();
                break;
            }
            "--id" => id = Some(words.next()?.clone()),
            "--in" => in_secs = pns::parse_count(words.next()?)?,
            "--every" => every = Some(pns::parse_count(words.next()?)?),
            "--unless-marker" => marker = Some(words.next()?.clone()),
            "--until" => {
                let raw = words.next()?;
                until = Some(match raw.strip_prefix('+') {
                    Some(seconds) => Until::FromNow(pns::parse_count(seconds)?),
                    None => Until::Epoch(pns::parse_count(raw)?),
                });
            }
            _ => return None,
        }
    }
    (!args.is_empty()).then_some(ScheduleRequest {
        id: id?,
        in_secs,
        every,
        until,
        marker,
        args,
    })
}

/// `pns daemon cancel --id <id>`: forget one job.
fn daemon_cancel() -> i32 {
    let argv: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|word| word.to_string_lossy().into_owned())
        .collect();
    let [flag, id] = argv.as_slice() else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    if flag != "--id" {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    match pns::daemon::cancel(&state_dir(), id) {
        Ok(true) => {
            println!("pns daemon: cancelled `{id}`");
            0
        }
        // NOT AN ERROR. The end state the operator asked for is the one they
        // already have, and a non-zero exit here would make a drill's cleanup
        // step fail the second time it ran.
        Ok(false) => {
            println!("pns daemon: no job named `{id}` was scheduled");
            0
        }
        Err(refusal) => {
            eprintln!("pns daemon: {refusal}");
            1
        }
    }
}
