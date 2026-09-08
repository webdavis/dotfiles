use crate::LampBridge;
use pns_domain::{doctor::LightsReport, lamps::config::Lights};

pub enum DoctorBridge<B> {
    Missing,
    Disabled,
    Unconfigured,
    Ready(B),
}

/// What the doctor can say about the lamps, and the ONE place that decides
/// which of its five states this machine is in.
///
/// THE BRIDGE IS DIALLED HERE, and only here, and only for a config that has
/// asked for the lamps AND enabled hue AND named a bridge. It costs the three
/// listings the routing resolves from, whatever the map says: arbitration and
/// the dim window are per lamp, so the joins are needed by every config that
/// routes anything at all.
///
/// BEHIND THE PANIC BOUNDARY every other bridge call gets, for `pulse_outcome`'s
/// reason: a panicking call must cost this section its lines rather than end
/// the report where the operator reads it as complete. A call that panicked
/// resolved no lamp, which is what the unreachable line says.
///
/// THE COST, NAMED: each GET is bounded by `BRIDGE_DEADLINE`, so a bridge that
/// accepts and never answers adds up to thirty seconds to `pns doctor`. That is
/// the same order as the pairing check's own two deadlines and it is paid only
/// by a machine that wrote the table.
pub fn doctor_lamps<B: LampBridge>(
    lights: Option<&Lights>,
    bridge: impl FnOnce() -> DoctorBridge<B>,
) -> LightsReport {
    let Some(lights) = lights else {
        return LightsReport::Off;
    };
    let bridge = match bridge() {
        DoctorBridge::Missing => return LightsReport::HueMissing,
        DoctorBridge::Disabled => return LightsReport::HueDisabled,
        DoctorBridge::Unconfigured => return LightsReport::NoBridge,
        DoctorBridge::Ready(bridge) => bridge,
    };
    let resolved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        bridge
            .inventory()
            .map(|inventory| pns_domain::lamps::resolve(&inventory, lights))
    }));
    match resolved {
        Ok(Some(map)) => LightsReport::Resolved(map),
        Ok(None) | Err(_) => LightsReport::Unreachable,
    }
}
