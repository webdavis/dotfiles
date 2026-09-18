use crate::*;

pub(crate) fn clear_held_lamps(settings: Option<&toml::Table>) {
    pns_application::clear_held_lamps(&pns_adapters::SqliteStore::for_records(state_dir()), || {
        let hue = settings.and_then(armed_hue_settings)?;
        Some(pns_adapters::TypedLampBridge(UreqBridge::new(
            &hue,
            BRIDGE_DEADLINE,
        )))
    });
}
