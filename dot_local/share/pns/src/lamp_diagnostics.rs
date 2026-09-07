use crate::*;

/// Whether the config's hue table resolves to a bridge that could be dialled:
/// the same reading `fire_pulse` takes, taken BEFORE it, so a check can tell a
/// bridge that listed no room from a config that names no bridge at all.
pub(crate) fn hue_resolves(hue_table: Option<&toml::Table>) -> bool {
    hue_table.is_some_and(|settings| {
        hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()).is_some()
    })
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
pub(crate) fn lights_report(
    lights: Option<&pns::config::Lights>,
    hue_table: Option<&toml::Table>,
    hue_declared: bool,
) -> pns::doctor::LightsReport {
    let Some(lights) = lights else {
        return pns::doctor::LightsReport::Off;
    };
    let Some(settings) = hue_table else {
        // NEVER WRITTEN AND SWITCHED OFF ARE DIFFERENT CONFIGS, and the
        // enabled table is one `None` for both, so the declaration is read
        // separately rather than inferred from its absence.
        return if hue_declared {
            pns::doctor::LightsReport::HueDisabled
        } else {
            pns::doctor::LightsReport::HueMissing
        };
    };
    let Some(hue) = hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref()) else {
        return pns::doctor::LightsReport::NoBridge;
    };
    let resolved = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        pns::channels::hue::resolve_on_bridge(
            &UreqBridge {
                base: format!("https://{}/clip/v2/resource", hue.bridge),
                key: hue.key,
                deadline: BRIDGE_DEADLINE,
            },
            lights,
        )
    }));
    match resolved {
        Ok(Some(map)) => pns::doctor::LightsReport::Resolved(map),
        Ok(None) | Err(_) => pns::doctor::LightsReport::Unreachable,
    }
}
/// The pulse behind the same boundary every leg gets, so a panicking bridge
/// call costs the census the rest of its lines rather than ending the report
/// where the operator reads it as complete.
pub(crate) fn pulse_outcome(hue_table: Option<toml::Table>) -> pns::doctor::Outcome {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        fire_pulse(hue_table, pns::config::Behaviour::Done)
    })) {
        Ok(rooms) => pns::doctor::Outcome::Signalled(rooms),
        // NO ROOM IS CLAIMED, and no panic text is quoted: the message is
        // written for a developer and may hold anything the pulse was carrying.
        Err(_) => {
            pns::doctor::Outcome::Failed("the pulse PANICKED; no room was signalled".to_string())
        }
    }
}

#[cfg(test)]
#[path = "lamp_diagnostics/tests.rs"]
mod lamp_diagnostics_tests;
