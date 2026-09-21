use crate::*;
use pns_application::JobChildren;

pub(crate) fn daemon_run() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    }
    pns_adapters::catch_termination();
    // The first tick sweeps, so a gateway that has just started clears
    // whatever the machine accumulated while it was down.
    let mut next_sweep = 0;
    // The first pass runs at once, so a gateway that starts after a window
    // ended writes that window's summary rather than waiting for the next.
    let mut next_pregenerate = 0;
    let mut pregenerated: std::collections::HashMap<String, u64> = std::collections::HashMap::new();
    pns_application::RunDaemon {
        settings: &pns_adapters::DaemonConfig {
            home: std::env::var("HOME").unwrap_or_default(),
        },
        clock: &now_secs,
        stopping: &pns_adapters::stopping,
    }
    .run(
        || {
            let state = state_dir();
            if let pns_adapters::job_spool::Startup::Refused(refusal) =
                pns_adapters::job_spool::prepare_spool(&state)
            {
                return Err(refusal);
            }
            let tick = pns_application::daemon_tick(
                std::env::var("PNS_DAEMON_TICK_INTERVAL").ok().as_deref(),
            );
            Ok((
                pns_adapters::FileJobSpool::new(state),
                pns_adapters::DaemonChildren::new(tick),
                move || std::thread::sleep(tick),
            ))
        },
        |now, children| {
            prune_activity(now, &mut next_sweep);
            crate::command_gateway::pregenerate::pregenerate(
                now,
                children,
                &mut next_pregenerate,
                &mut pregenerated,
            )?;
            start_retry(now, children)?;
            start_page(now, children)
        },
        |notice| match notice {
            pns_application::DaemonNotice::Output(line) => println!("{line}"),
            pns_application::DaemonNotice::Error(line) => eprintln!("{line}"),
        },
    )
}

/// Delete the activity rows that have outlived `[recap] retain`.
///
/// ONCE AN HOUR, NOT ONCE A TICK. The retention is measured in days, so a
/// sweep every second would open the database 3,600 times an hour to delete
/// nothing; an hour late on a thirty-day boundary is not late.
///
/// THE CONFIG IS READ AT THE SWEEP, like `start_page`'s own read, because the
/// gateway outlives an edit: a shortened retention takes effect on the next
/// sweep with no bounce.
///
/// AN UNREADABLE CONFIG KEEPS THE DEFAULT rather than deleting nothing: the
/// table has no off switch, and a file that will not parse must not turn the
/// store into a log that grows for good.
fn prune_activity(now: u64, next_sweep: &mut u64) {
    if now < *next_sweep {
        return;
    }
    *next_sweep = now.saturating_add(SWEEP_INTERVAL_SECS);
    let home = std::env::var("HOME").unwrap_or_default();
    let retain = match pns_adapters::load_config(&pns_adapters::config_path(&home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config.recap.retain,
        _ => pns_domain::recap::Recap::default().retain,
    };
    let Some(cutoff) = now.checked_sub(retain.as_secs()) else {
        return;
    };
    let store = pns_adapters::SqliteStore::new(state_dir());
    if let Err(error) = store.prune_activity(cutoff) {
        eprintln!("pns gateway: the activity store could not be pruned: {error}");
    }
    // THE PARAGRAPHS GO WITH THE EVENTS THEY ARE ABOUT, under the one
    // retention: a summary of a window whose rows have gone answers a question
    // nothing else in the store can still back.
    if let Err(error) = store.prune_recap_summaries(cutoff) {
        eprintln!("pns gateway: the stored recap summaries could not be pruned: {error}");
    }
}

/// How long between two sweeps of the activity store. See `prune_activity`.
const SWEEP_INTERVAL_SECS: u64 = 3600;

fn start_retry(now: u64, children: &mut impl JobChildren) -> Result<(), String> {
    // A leading dot cannot be a scheduled job id, so producer jobs cannot
    // suppress this child or be mistaken for an already-running retry.
    const RETRY: &str = ".delivery-retry";
    if children.running(RETRY) {
        return Ok(());
    }
    children.start(&pns_domain::jobs::Job {
        id: RETRY.into(),
        due: now,
        until: now,
        every: None,
        unless_marker: None,
        args: vec!["gateway".into(), "retry".into()],
    })
}

/// The failure page, as a child of the clock.
///
/// A LONG-LIVED CHILD IN A SET BUILT FOR SHORT ONES, and that is what supervises
/// it: `running` keeps a second listener from ever starting, and a listener that
/// dies takes the page down only until the next tick.
///
/// THE CONFIG IS READ EVERY TICK rather than once at start-up, because the
/// daemon outlives an edit. A page switched off stops being restarted, and one
/// switched on is up within a tick, with no bounce of the daemon in either
/// direction.
fn start_page(now: u64, children: &mut impl JobChildren) -> Result<(), String> {
    const PAGE: &str = pns_domain::jobs::PAGE_JOB;
    if children.running(PAGE) {
        return Ok(());
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let page_enabled = match pns_adapters::load_config(&pns_adapters::config_path(&home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config.failures.page_enabled,
        // AN UNREADABLE CONFIG SERVES NOTHING. Every other default in this file
        // errs toward doing the thing, but this one opens a socket, and a
        // listener nobody has asked for is not what a broken file should
        // produce.
        _ => false,
    };
    if !page_enabled {
        return Ok(());
    }
    children.start(&pns_domain::jobs::Job {
        id: PAGE.into(),
        due: now,
        until: now,
        every: None,
        unless_marker: None,
        args: vec!["failures".into(), "serve".into()],
    })
}

pub(crate) fn daemon_retry() -> i32 {
    if !crate::arguments_after_verb().is_empty() {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    }
    let result = now_secs()
        .ok_or_else(|| "the clock is unavailable".to_string())
        .and_then(crate::delivery_runtime::retry_pending);
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("pns gateway: delivery retry failed: {error}");
            1
        }
    }
}
