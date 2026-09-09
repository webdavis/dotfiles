use crate::*;
use pns_adapters::SqliteStore;

/// The `quiet` mode: the operator's own mute, typed and timed.
///
/// THE ONLY NON-ZERO EXITS HERE THAT ARE NOT AN OPERATOR'S APPROVAL DECISION,
/// and they are correct. The always-exit-0 contract covers the hook and
/// notification paths, where a non-zero exit would fail the turn being
/// reported on; this is hand typed, is never a hook, and a subcommand that
/// silently swallows a typo is a mute the operator believes is on.
///
/// THE REPORT IS READ BACK FROM THE REPOSITORY after whatever was asked for, rather
/// than rendered from what this run intended, so the line cannot claim a mute
/// that never landed. A FAILED SET REPORTS TOO, for the mirror of the same
/// reason: it knows only that its own write did not happen, and a previous
/// mute may still be standing behind it.
pub(crate) fn quiet_mode() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_subcommand();
    let records = SqliteStore::for_records(state_dir());
    // A SET THAT DID NOT HAPPEN, carried to the exit code rather than
    // returned on the spot, so the report below runs on this path too.
    let mut set_failed = false;
    match arguments.as_slice() {
        // NO ARGUMENT REPORTS and mutes nothing. There is no untimed toggle:
        // an indefinite mute the operator forgets is a notification system
        // that has silently stopped working, and making this form the report
        // also means no invocation can mute by accident.
        [] => {}
        // Clearing also replaces an imported record nothing could parse. The
        // standing-state report below still decides what actually happened.
        [word] if word == "off" => {
            let _ = records.set_quiet_expiry(None);
        }
        [duration] => match pns_domain::quiet::parse_duration(duration) {
            Ok(seconds) => {
                // NEITHER ARM CLAIMS "nothing is muted". A run that could not
                // read a clock or could not write cannot see the state it is
                // making a claim about, and a mute set an hour ago can be
                // standing behind both: measured, the write arm said nothing
                // was muted while `pns quiet` a second later reported sixty
                // minutes left. They say what did not happen, and the report
                // below says what stands.
                match now_secs().map(|now| now.saturating_add(seconds)) {
                    None => {
                        eprintln!(
                            "pns: state error (the clock cannot be read); the mute was not set"
                        );
                        set_failed = true;
                    }
                    // LOUD, unlike `remember_staleness`: that one is a
                    // background warning that must never crash a diagnostic,
                    // and this is a human waiting on an answer. Reporting
                    // success for a mute that is not in effect is the worst
                    // outcome available.
                    Some(expiry) => {
                        if let Err(error) = records.set_quiet_expiry(Some(expiry)) {
                            eprintln!(
                                "pns: state error (quiet-until could not be written: {error}); \
                                 the mute was not set"
                            );
                            set_failed = true;
                        }
                    }
                }
            }
            Err(refusal) => {
                eprintln!("{refusal}");
                eprintln!("{QUIET_USAGE}");
                return 2;
            }
        },
        // ANY EXTRA WORD IS A REFUSAL, never a silent fallthrough to the
        // report: a typo an operator does not see is a mute they believe is
        // on.
        _ => {
            eprintln!("{QUIET_USAGE}");
            return 2;
        }
    }
    println!(
        "{}",
        pns_domain::quiet::status_line(read_quiet_expiry(&records), now_secs())
    );
    if set_failed { 1 } else { 0 }
}

/// What a mute typed wrong is told, once, on stderr. The refusal above it
/// quotes what was typed; this says what the command takes.
const QUIET_USAGE: &str =
    "pns: usage: pns quiet [<duration>|off]; duration is <count><s|m|h>, from 1s to 24h";

/// Whether the operator's mute is on, judged on THE RUN'S OWN clock reading:
/// the same one the rest of the decision is taken against. An expiry crossed
/// mid-run costs one event either way, and one decision on one reading is the
/// engine's stated contract.
pub(crate) fn muted_now(now_secs: Option<u64>) -> bool {
    pns_domain::quiet::is_muted(
        read_quiet_expiry(&SqliteStore::for_records(state_dir())),
        now_secs,
    )
}

fn read_quiet_expiry(records: &SqliteStore) -> Option<u64> {
    match records.quiet_expiry() {
        Ok(expiry) => expiry,
        Err(pns_adapters::StoreError::InvalidState(complaint)) => {
            eprintln!("{complaint}");
            None
        }
        Err(error) => {
            eprintln!(
                "pns: state error (quiet-until could not be read: {error}); nothing is muted, clear it with pns quiet off"
            );
            None
        }
    }
}
