use crate::*;
use pns_application::{ScheduleJob, Until};

// --- the daemon -------------------------------------------------------------

/// `pns daemon <verb>`: the clock, and the two typed commands that feed it.
///
/// A BARE `pns daemon` IS A REFUSAL, per the house rule that an unknown
/// argument never falls through to help with exit 0: a verb this does not serve
/// is a command the operator believes ran.
pub(crate) fn daemon_mode(verb: &str) -> i32 {
    match verb {
        "run" => daemon_run(),
        "retry" => crate::daemon_runtime::daemon_retry(),
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
    let argv: Vec<String> = crate::arguments_after_verb();
    let Some(request) = parse_schedule(&argv) else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    match request.run(&pns_adapters::FileJobSpool::new(state_dir()), now_secs()) {
        Ok(()) => 0,
        Err(refusal) => {
            eprintln!("pns daemon: {refusal}");
            1
        }
    }
}

/// The typed request, or None for anything this will not run.
///
/// UNKNOWN IS AN ERROR, never a silent skip: `pns`'s own event parser is
/// lenient because it sits on a notification path that must not fail, and this
/// one sits in front of an operator who typed a command and will believe it
/// did what they wrote.
fn parse_schedule(argv: &[String]) -> Option<ScheduleJob> {
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
            "--in" => in_secs = pns_domain::count::parse_count(words.next()?)?,
            "--every" => every = Some(pns_domain::count::parse_count(words.next()?)?),
            "--unless-marker" => marker = Some(words.next()?.clone()),
            "--until" => {
                let raw = words.next()?;
                until = Some(match raw.strip_prefix('+') {
                    Some(seconds) => Until::FromNow(pns_domain::count::parse_count(seconds)?),
                    None => Until::Epoch(pns_domain::count::parse_count(raw)?),
                });
            }
            _ => return None,
        }
    }
    (!args.is_empty()).then_some(ScheduleJob {
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
    let argv: Vec<String> = crate::arguments_after_verb();
    let [flag, id] = argv.as_slice() else {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    };
    if flag != "--id" {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    match pns_application::cancel_job(&pns_adapters::FileJobSpool::new(state_dir()), id) {
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
