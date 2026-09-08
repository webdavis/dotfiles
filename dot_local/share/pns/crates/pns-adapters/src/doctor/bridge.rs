use crate::{BRIDGE_DEADLINE, TypedLampBridge, UreqBridge, hue_settings};
use pns_application::DoctorBridge;

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
pub fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()).is_some()
    })
}
pub fn doctor_bridge(
    table: Option<&toml::Table>,
    declared: bool,
) -> DoctorBridge<TypedLampBridge<UreqBridge>> {
    let Some(table) = table else {
        return if declared {
            DoctorBridge::Disabled
        } else {
            DoctorBridge::Missing
        };
    };
    let Some(hue) = hue_settings(table, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return DoctorBridge::Unconfigured;
    };
    DoctorBridge::Ready(TypedLampBridge(UreqBridge {
        base: format!("https://{}/clip/v2/resource", hue.bridge),
        key: hue.key,
        deadline: BRIDGE_DEADLINE,
    }))
}

#[cfg(test)]
#[path = "bridge/tests.rs"]
mod lamp_diagnostics_tests;
