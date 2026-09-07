use crate::*;

/// `pns loop begin|end`: take the loop lamp by hand, and give it back.
///
/// THE LEASE IS THE SECOND TRIGGER, beside the automatic one, and it exists for
/// work whose length nothing can measure in advance: an overnight run is a loop
/// from the moment it starts, not once it has been going five minutes.
///
/// IT WRITES A FILE AND REGISTERS THE TICK. The tick is what reads the lease,
/// and its own lease is refreshed by EVENT traffic: a lease taken by hand in a
/// pane that then goes quiet for an hour would be read by nobody, because the
/// tick would have expired minutes into the run it was taken for. A daemon that
/// is down still means the lamp simply does not light, and `pns loop end` on a
/// machine that never began is a removal of a file that is not there.
pub(crate) fn loop_mode(verb: &str) -> i32 {
    let arguments: Vec<String> = std::env::args_os()
        .skip(3)
        .map(|argument| argument.to_string_lossy().into_owned())
        .collect();
    let command = match pns::lights::loop_command(
        verb,
        &arguments,
        std::env::var("HERDR_PANE_ID").ok().as_deref(),
    ) {
        Ok(command) => command,
        Err(refusal) => {
            eprintln!("{refusal}");
            return 2;
        }
    };
    let state = state_dir();
    match command {
        pns::lights::LoopCommand::Begin(pane) => {
            // NO CLOCK IS NO LEASE, never a lease at epoch zero: the timeout is
            // measured against this number, and a zero would be expired the
            // moment it was written.
            let (Some(marker), Some(now)) = (pns::lights::lease_marker(&state, &pane), now_secs())
            else {
                eprintln!("pns: loop: the clock cannot be read; the lease was not taken");
                return 1;
            };
            if let Err(error) = publish_state_line(&marker, &now.to_string()) {
                // LOUD, because a human is waiting on the answer: a lease that
                // was not taken is a lamp that never lights, and reporting
                // success for one is the worst outcome available.
                eprintln!("pns: loop: the lease could not be written: {error}");
                return 1;
            }
            // AND THE TICK IS REGISTERED FOR THE WHOLE LEASE, because nothing
            // else will register it in time. The tick's own lease is refreshed
            // by EVENT traffic, so a lease taken by hand in a pane that then
            // goes quiet, which is exactly the overnight run this verb exists
            // for, would be read by a tick that expired minutes into it.
            let home = std::env::var("HOME").unwrap_or_default();
            if let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home))
                && let Some(lights) = config.lights.as_deref()
            {
                schedule_lights_tick(&state, lights, now, lights.looping.lease_timeout_secs);
            }
        }
        pns::lights::LoopCommand::End(pane) => {
            if let Err(refusal) = end_lease(&state, &pane) {
                eprintln!("{refusal}");
                return 1;
            }
        }
    }
    0
}

/// Give a lease back, or say why it could not be given back.
///
/// LOUD, because a human is waiting on the answer and the lamp is a liveness
/// signal: reporting that a loop has ended while its lease is still on disk
/// leaves the loop lamp breathing for the whole timeout with nothing behind it,
/// and the operator has been told the opposite.
///
/// A LEASE THAT IS NOT THERE IS NOT A FAILURE. `pns loop end` on a machine that
/// never began, or a second one after the first, is a removal of a file that is
/// already gone, which is exactly the state the command is for.
fn end_lease(state: &Path, pane: &str) -> Result<(), String> {
    let Some(marker) = pns::lights::lease_marker(state, pane) else {
        return Ok(());
    };
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "pns: loop: the lease could not be given back ({error}); the loop lamp \
             keeps breathing until it times out"
        )),
    }
}

/// Renew the lease this pane holds, if it holds one.
///
/// THE PANE'S ORDINARY HOOK TRAFFIC IS THE RENEWAL, which is what makes the
/// lease a liveness signal rather than a timer: an agent that is still working
/// is still firing events from its own pane, and one that stopped stops
/// renewing. Nothing else in this crate renews it.
///
/// IT CREATES NOTHING, and that is a property of the WRITE rather than of a
/// check in front of one. The open states no `create`, so the file has to be
/// there already, and the bytes go through the HANDLE rather than through a
/// fresh file renamed over the path: a `pns loop end` that lands after the open
/// sends these bytes to an inode nobody can reach any more, where a look-then-
/// publish would have written the lease back into existence and left the lamp
/// breathing for a whole timeout over work that had finished.
///
/// IT WRITES IN PLACE RATHER THAN TRUNCATING FIRST, so a tick reading the file
/// mid-renewal cannot see an empty one and sweep the lease. Both epochs are ten
/// digits and will be for the next two centuries, so a read caught between the
/// two sees a mix of two same-length numbers, which is a second or two out
/// rather than a lease nobody can parse. The `set_len` after the write is for
/// the day that stops being true.
pub(crate) fn renew_loop_lease(state: &Path, pane: &str, now: Option<u64>) {
    let (Some(marker), Some(now)) = (pns::lights::lease_marker(state, pane), now) else {
        return;
    };
    // The failures are DROPPED here: a lease that did not renew costs the lamp
    // one timeout, and this process has no reader for a complaint.
    let line = format!("{now}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&marker)
        && file.write_all(line.as_bytes()).is_ok()
    {
        let _ = file.set_len(line.len() as u64);
    }
}

#[cfg(test)]
#[path = "command_loop/tests.rs"]
mod command_loop_tests;
