use crate::{LoadOutcome, config_path, env_deadline, load_config};
use std::time::Duration;

/// How long that wait may last: the test hatch, then the operator's own
/// `[plugins.mobile] submit_deadline_secs`, then the default.
pub(super) fn submit_deadline() -> Duration {
    // A LITERAL ZERO IS NOT A BOUND, it is this wait switched off by accident,
    // and the config layer already refuses one by name for that reason. The
    // refusal sits here rather than in `env_deadline`, which keeps the
    // accepted semantics the payload hatch shares with it: a zero here falls
    // through to the config, exactly as an unset variable would.
    env_deadline("PNS_MOSHI_SUBMIT_DEADLINE_MS")
        .filter(|deadline| !deadline.is_zero())
        .unwrap_or_else(configured_submit_deadline)
}
/// The configured bound, and the DEFAULT for every way of not stating one.
///
/// A config that is absent or unreadable asked for nothing, which is the
/// default; a config that states a value this layer refuses says so OUT LOUD
/// and then takes the default too, because an operator who asked for something,
/// did not get it and was told nothing is the defect one level down.
fn configured_submit_deadline() -> Duration {
    let home = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => config,
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
