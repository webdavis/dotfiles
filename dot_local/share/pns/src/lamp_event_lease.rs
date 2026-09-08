use crate::*;

pub(crate) fn clear_held_lamps(settings: Option<&toml::Table>) {
    pns_application::clear_held_lamps(&pns_adapters::SqliteStore::for_records(state_dir()), || {
        let hue = settings.and_then(|settings| {
            hue_settings(settings, std::env::var("HUE_PULSE_ROOMS").ok().as_deref())
        })?;
        Some(pns_adapters::TypedLampBridge(UreqBridge {
            base: format!("https://{}/clip/v2/resource", hue.bridge),
            key: hue.key,
            deadline: BRIDGE_DEADLINE,
        }))
    });
}
