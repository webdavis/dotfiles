use super::{delivery_notice, lease};
use crate::{
    LoadOutcome, Mobile, channel_dispatch, config_path, hermes_secret, load_config, now_secs,
    plugin_settings, read_mobile, roster, select_plugins, state_dir,
};
use pns_adapters::SqliteStore;
use pns_application::{DeliveryLedger, SubmissionDelivery};

pub(crate) fn retry_pending(now: u64) -> Result<(), String> {
    let store = SqliteStore::for_records(state_dir());
    let home = std::env::var("HOME").unwrap_or_default();
    let loaded = load_config(&config_path(&home));
    let (limits, backoff) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (config.retry_limits, config.retry_backoff),
        _ => Default::default(),
    };
    let (mobile, hermes_key) = match &loaded {
        Ok(LoadOutcome::Loaded(config)) => (
            read_mobile(config),
            plugin_settings(config, "hermes").and_then(hermes_secret),
        ),
        _ => (Mobile::default(), None),
    };
    let (selection, _) = select_plugins(&roster(), loaded);
    retry_once(
        &store,
        now,
        limits,
        |message| alarm_banner(message, pns_adapters::SystemCommandRunner),
        |retry, window| {
            let destinations = channel_dispatch::destinations(
                &selection,
                &retry.leg.route,
                &home,
                &mobile,
                hermes_key,
            );
            SubmissionDelivery {
                ledger: &store,
                decisions: &store,
                destinations: &destinations,
            }
            .attempt_retry(retry, window, backoff, &now_secs, &delivery_notice);
        },
    )
}

fn retry_once(
    store: &SqliteStore,
    now: u64,
    limits: pns_domain::retry::RetryLimits,
    banner: impl FnOnce(&str) -> pns_domain::Delivery,
    attempt: impl FnOnce(
        pns_application::RetryDelivery<pns_adapters::DeliveryClaim>,
        pns_application::LeaseWindow,
    ),
) -> Result<(), String> {
    let window = lease(now).map_err(|_| "delivery retry clock overflow".to_string())?;
    let claimed = store.claim_retry(window, limits);
    let claimed = claimed.map(|retry| {
        if let Some(retry) = retry {
            attempt(retry, window);
            // THE DEAD-LETTER IS WHAT THIS PASS ANNOUNCES. A leg's first failure
            // happened on the submission path and was announced there; what only
            // this loop ever sees is the attempt that spends the last one.
            crate::failure_notice::announce(store, window.now);
        }
    });
    let health = match &claimed {
        Ok(_) => store.sample_delivery_health(),
        Err(_) => Err(pns_application::LedgerFailure::Unavailable(
            "delivery retry storage unavailable".into(),
        )),
    };
    let alarm = pns_application::report_delivery_health(health.as_ref(), banner, |generation| {
        store.acknowledge_delivery_alarm(generation)
    });
    if alarm.is_err() {
        store.report_delivery_health_failure();
    }
    claimed.map_err(|_| "delivery retry storage unavailable".to_string())?;
    alarm
}

fn alarm_banner(
    message: &str,
    runner: impl pns_application::CommandRunner + Send + Sync,
) -> pns_domain::Delivery {
    use pns_application::NotificationDestination;
    let event = channel_dispatch::rendered_event_quiet(
        &pns_domain::EventArgs {
            agent: "pns".into(),
            state: "blocked".into(),
            project: "delivery pipeline degraded".into(),
            detail: message.into(),
            ..Default::default()
        },
        false,
    );
    pns_adapters::BannerChannel {
        runner,
        terminal_id: String::new(),
        herdr_path: None,
    }
    .deliver(&pns_application::DeliveryRequest {
        producer: "pns",
        request_id: None,
        event: &event,
        route: "",
        mode: pns_domain::routing::ReportMode::Silent,
    })
}

#[cfg(test)]
mod tests;
