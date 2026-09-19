use crate::{LoadOutcome, config_path, load_config};
use std::time::Duration;

/// How long that wait may last: the operator's own
/// `[plugins.mobile] submit_deadline_secs`, else the default.
///
/// A config that is absent or unreadable asked for nothing, which is the
/// default; a config that states a value this layer refuses says so OUT LOUD
/// and then takes the default too, because an operator who asked for something,
/// did not get it and was told nothing is the defect one level down.
pub(super) fn submit_deadline() -> Duration {
    let home = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => *config,
        _ => crate::config::Config::default(),
    };
    crate::config::submit_deadline(&config).unwrap_or_else(|error| {
        eprintln!(
            "pns: config error ({}); the moshi submission keeps its {}-second bound",
            error.detail(),
            crate::config::DEFAULT_SUBMIT_DEADLINE_SECS
        );
        Duration::from_secs(crate::config::DEFAULT_SUBMIT_DEADLINE_SECS)
    })
}
