use crate::*;

/// The home probe, as the doctor's own rows: one reading of the router, the
/// evidence behind it, and the one stale-identifier alert that reading may
/// earn.
///
/// A DIAGNOSTIC FIRST: it says what it found, including every way it can be
/// unconfigured, because its job is to answer "why did the probe not read" as
/// much as "is the device home". The key itself is never printed, on any path.
///
/// NOTHING HERE IS GRADED `Bad`, so the reading cannot move the doctor's exit
/// code. See `home_report::verdict_mark`.
///
/// AND THE TRIGGER for the stale-identifier alert, on exactly the condition
/// that prints the warning. This is the only code that reads the sensor and it
/// already holds the derive/decide/remember trio, so one call site keeps ONE
/// memory and ONE decision; a second entrypoint would be a second place for
/// the episode decision to fall out of step. The consequence is deliberate: a
/// hand-run report no longer consumes an episode silently, it delivers it.
pub(crate) fn rows() -> Vec<pns_domain::doctor::Item> {
    use crate::{home_report as report, home_setup_row as setup_row};
    use pns_adapters::SetupFailure;
    // A DELIBERATE SECOND READ, not a missed reuse. `DoctorActions::home` is a
    // bare `fn() -> Vec<Item>` rather than a captured closure (see that
    // field's doc comment), and by the time it runs, `command_doctor.rs` has
    // already moved its own loaded config into `select_plugins`, so there is
    // no borrow left to hand this section. `$HOME` does not change within a
    // process and `load_config` is a pure read of the file at the path it
    // derives from it, so this section sees the same config the rest of the
    // report did unless the file is edited between the two reads, which is
    // the same race every doctor section already runs against the config on
    // disk.
    let home_dir = std::env::var("HOME").unwrap_or_default();
    let config = match load_config(&config_path(&home_dir)) {
        Ok(LoadOutcome::Loaded(config)) => config,
        Ok(LoadOutcome::Missing) => return vec![setup_row(&SetupFailure::NoConfigFile)],
        Err(error) => {
            return vec![setup_row(&SetupFailure::ConfigError(
                error.detail().to_string(),
            ))];
        }
    };
    // EVERY CAUSE IS DECIDED IN THE LIBRARY, so each line is pinned by a
    // value-in, value-out test and this stays wiring: a missing table, a
    // disabled one, a `type` nothing answers and a mistyped value each send
    // the operator to a different edit, and one message covering two of them
    // sends half of them to the wrong one.
    let router_table = match pns_adapters::enabled_router_table(&config) {
        Ok(table) => table,
        Err(failure) => return vec![setup_row(&failure)],
    };
    // WHERE THE ALERT GOES, settled at the config read rather than at the
    // post. `hermes_target`'s own refusal names `--route`, a flag nobody
    // typed on this path; this one names the key in the file, and it is said
    // on every run of the report instead of only on the run that happens
    // to have something to deliver.
    let (alert_route, complaint) = pns_adapters::stale_alert_route(router_table);
    if let Some(complaint) = complaint {
        eprintln!("{complaint}");
    }
    let settings = match pns_adapters::router_settings(router_table) {
        Ok(settings) => settings,
        Err(failure) => return vec![setup_row(&failure)],
    };
    // The key stays its own read, so it never joins the settings in a type
    // that could be dumped whole.
    let Some(key) = pns_adapters::router_api_key(router_table) else {
        return vec![setup_row(&SetupFailure::NoApiKey)];
    };
    let router = pns_adapters::UniFiRouter::new(settings.url, key);
    let mut rows = Vec::new();
    pns_application::ReadHomeProbe {
        router: &router,
        memory: &pns_adapters::SqliteStore::for_records(state_dir()),
        notifier: &HomeNotification,
    }
    .run(&settings.device, alert_route, |reading, alert| {
        rows = report(reading, alert);
    });
    rows
}

struct HomeNotification;
impl pns_application::RaiseNotification for HomeNotification {
    fn raise(&self, event: &pns_domain::EventArgs) {
        run_event(
            event,
            &system_probes(),
            &HookPayload::default(),
            Attempt::First,
        );
    }
}
