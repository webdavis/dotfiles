use crate::*;
use pns_application::{RECAP_USAGE, recap_bounds};

/// The `recap` mode: one window of activity, rendered and posted, in a process
/// nobody is waiting on.
///
/// IT TAKES NO DECISION, which is what makes it a mode. The decision was taken
/// by the event that spawned it, and re-deciding here would be the second
/// reading of one moment `GateInputs` exists to forbid.
///
/// IT REACHES ONE DESTINATION, the durable route, and never the phone or the
/// banner. The phone layer was already delivered by the card that pointed here.
///
/// EXIT 2 FOR A MISTYPED INVOCATION, in `quiet_mode`'s style rather than the
/// hook path's always-zero: this is hand-runnable, and a subcommand that
/// swallows a typo is a recap the operator believes was posted. The spawner
/// never reads the code.
pub(crate) fn recap_mode() -> i32 {
    pns_adapters::run_recap_bounded(recap)
}

fn recap() -> i32 {
    let arguments: Vec<String> = crate::arguments_after_subcommand();
    let Some((since, until)) = recap_bounds(&arguments) else {
        eprintln!("{RECAP_USAGE}");
        return 2;
    };
    let home = std::env::var("HOME").unwrap_or_default();
    // FAIL CLOSED ON THE ROUTE AND ON THE SUMMARIZER, AND OPEN ON THE POST,
    // which is `pulse_mode`'s split: a config nobody can read named no route
    // and no command, so the recap goes to the default route, plainly, rather
    // than to a route the operator never asked for or through a program they
    // never named.
    let (hermes_key, recap) = match load_config(&config_path(&home)) {
        Ok(LoadOutcome::Loaded(config)) => (
            plugin_settings(&config, "hermes").and_then(hermes_secret),
            config.recap,
        ),
        _ => (
            None,
            pns_adapters::Recap {
                digest_as_thread: false,
                ..Default::default()
            },
        ),
    };
    let body = pns_application::BuildReturnRecap {
        activity: &pns_adapters::SqliteStore::for_records(state_dir()),
        merges: &pns_adapters::GitHubMerges,
        notes: &pns_adapters::ReviewNotes { home: home.clone() },
        summarizer: &pns_adapters::ProcessSummarizer,
    }
    .run(
        &recap,
        since,
        until,
        |at| pns_application::recap_wall_clock(at, local_minutes_since_midnight),
        |budget| {
            let end = std::time::Instant::now() + budget;
            move || end.saturating_duration_since(std::time::Instant::now())
        },
    );
    pns_application::post_return_recap(&body, recap.digest_as_thread, |body, route| {
        crate::recap_delivery_runtime::deliver_recap(body, route, &home, hermes_key.clone())
            .into_iter()
            .map(|(_, outcome)| outcome)
            .collect()
    })
}
