use crate::{BRIDGE_DEADLINE, TypedLampBridge, UreqBridge, hue_settings};
use pns_application::DoctorBridge;

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
pub fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        matches!(
            hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()),
            Ok(Some(_))
        )
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
    let Some(hue) = crate::armed_hue(
        table,
        std::env::var("HUE_PULSE_ROOMS").ok().as_deref(),
        |refusal| eprintln!("{refusal}"),
    ) else {
        return DoctorBridge::Unconfigured;
    };
    DoctorBridge::Ready(TypedLampBridge(UreqBridge::new(&hue, BRIDGE_DEADLINE)))
}

#[cfg(test)]
#[path = "bridge/tests.rs"]
mod lamp_diagnostics_tests;
