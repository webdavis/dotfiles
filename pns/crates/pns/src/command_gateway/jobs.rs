use super::GATEWAY_USAGE;
use crate::*;
use pns_application::{ScheduleJob, Until};

/// `pns gateway schedule`: one registration, typed.
///
/// FOR DRILLS AND FOR TESTS. The library function beneath it is what a rider
/// will call, in-process, so nothing ever spawns a process to talk to the
/// daemon.
pub(super) fn gateway_schedule() -> i32 {
    let argv: Vec<String> = crate::arguments_after_verb();
    let Some(request) = parse_schedule(&argv) else {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    };
    match request.run(&pns_adapters::FileJobSpool::new(state_dir()), now_secs()) {
        Ok(()) => 0,
        Err(refusal) => {
            eprintln!("pns gateway: {refusal}");
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
            // ONLY THE RELATIVE FORM LIVES HERE: a point in time says so with
            // its own flag, `--until-epoch`, rather than a bare number this
            // one would have to guess the shape of.
            "--until" => {
                let raw = words.next()?.strip_prefix('+')?;
                until = Some(Until::FromNow(pns_domain::count::parse_count(raw)?));
            }
            "--until-epoch" => {
                until = Some(Until::Epoch(pns_domain::count::parse_count(words.next()?)?));
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

/// `pns gateway cancel --id <id>`: forget one job.
pub(super) fn gateway_cancel() -> i32 {
    let argv: Vec<String> = crate::arguments_after_verb();
    let [flag, id] = argv.as_slice() else {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    };
    if flag != "--id" {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    }
    match pns_application::cancel_job(&pns_adapters::FileJobSpool::new(state_dir()), id) {
        Ok(true) => {
            println!("pns gateway: cancelled `{id}`");
            0
        }
        // NOT AN ERROR. The end state the operator asked for is the one they
        // already have, and a non-zero exit here would make a drill's cleanup
        // step fail the second time it ran.
        Ok(false) => {
            println!("pns gateway: no job named `{id}` was scheduled");
            0
        }
        Err(refusal) => {
            eprintln!("pns gateway: {refusal}");
            1
        }
    }
}

#[cfg(test)]
mod schedule_until_tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn until_epoch_says_so_and_the_old_bare_number_is_refused() {
        let old = parse_schedule(&argv(&[
            "--id",
            "job",
            "--until",
            "1756500000",
            "--",
            "--state",
            "done",
        ]));
        assert!(
            old.is_none(),
            "a bare number after --until is not a duration"
        );

        let new = parse_schedule(&argv(&[
            "--id",
            "job",
            "--until-epoch",
            "1756500000",
            "--",
            "--state",
            "done",
        ]));
        assert!(matches!(
            new.expect("a valid schedule").until,
            Some(Until::Epoch(1_756_500_000))
        ));
    }

    #[test]
    fn until_stays_the_relative_form() {
        let relative = parse_schedule(&argv(&[
            "--id", "job", "--until", "+1800", "--", "--state", "done",
        ]));
        assert!(matches!(
            relative.expect("a valid schedule").until,
            Some(Until::FromNow(1800))
        ));
    }
}
