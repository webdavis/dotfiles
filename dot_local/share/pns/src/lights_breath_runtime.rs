use crate::*;
pub(crate) use pns_application::Breathing;
fn drive_breaths<B: pns_adapters::Bridge>(
    bridge: &B,
    budget_ms: u64,
    breathing: &[Breathing],
    elapsed_ms: impl FnMut() -> u64,
    sleep: impl FnMut(Duration),
) -> Vec<(String, u8, u64)> {
    pns_application::drive_breaths(
        &pns_adapters::TypedLampBridge(bridge),
        budget_ms,
        breathing,
        elapsed_ms,
        sleep,
    )
}

#[cfg(test)]
#[path = "lights_breath_runtime/tests.rs"]
mod lights_breath_runtime_tests;
