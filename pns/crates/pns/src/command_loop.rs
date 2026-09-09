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
    let arguments: Vec<String> = crate::arguments_after_verb();
    let command = match crate::loop_command(
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
    let leases = pns_adapters::FileLoopLeases::new(state.clone());
    let operation = pns_application::AcquireLoopLease { leases: &leases };
    let result = match command {
        crate::LoopCommand::Begin(pane) => operation.begin(&pane, now_secs(), |now| {
            let home = std::env::var("HOME").unwrap_or_default();
            if let Ok(LoadOutcome::Loaded(config)) = load_config(&config_path(&home))
                && let Some(lights) = config.lights.as_deref()
            {
                pns_application::schedule_lights_tick(
                    &pns_adapters::FileJobSpool::new(state.clone()),
                    lights,
                    now,
                    lights.looping.lease_timeout_secs,
                );
            }
        }),
        crate::LoopCommand::End(pane) => operation.end(&pane),
    };
    match result {
        Ok(()) => 0,
        Err(refusal) => {
            eprintln!("{refusal}");
            1
        }
    }
}
