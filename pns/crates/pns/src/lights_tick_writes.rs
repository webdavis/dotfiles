use crate::*;
const LIGHTS_TICK_LOCK: &str = "lights-tick.lock";
#[allow(clippy::too_many_arguments)]
fn run_tick_writes<B: pns_adapters::Bridge>(
    bridge: &B,
    state: &Path,
    lights: &pns_domain::lamps::config::Lights,
    active: &[pns_domain::lights::held::Held],
    reading: &pns_domain::lamps::Reading<'_>,
    held_before: Option<&[pns_domain::lights::phase::HeldEntry]>,
    now_ms: u64,
    presence: Option<&pns_domain::Snapshot>,
    elapsed_ms: impl FnMut() -> u64,
    sleep: impl FnMut(Duration),
) -> Vec<String> {
    let records = pns_adapters::SqliteStore::for_records(state.to_path_buf());
    pns_application::ReconcileLights {
        bridge: &pns_adapters::TypedLampBridge(bridge),
        held: &records,
        claim: &pns_adapters::FileLampTick(state.to_path_buf()),
        presence: &records,
    }
    .run(
        pns_application::TickReading {
            lights,
            active,
            reading,
            held_before,
            now_ms,
            presence,
        },
        elapsed_ms,
        sleep,
    )
}

#[cfg(test)]
#[path = "lights_tick_writes/tests/lifecycle.rs"]
mod lifecycle_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/budget.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/ownership.rs"]
mod ownership_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/phase.rs"]
mod phase_tests;

#[cfg(test)]
#[path = "lights_tick_writes/tests/routing.rs"]
mod routing_tests;
