use crate::*;
use pns_application::JobChildren;

pub(crate) fn daemon_run() -> i32 {
    if std::env::args_os().nth(3).is_some() {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    pns_application::RunDaemon {
        settings: &pns_adapters::DaemonConfig {
            home: std::env::var("HOME").unwrap_or_default(),
        },
        clock: &now_secs,
    }
    .run(
        || {
            let state = state_dir();
            if let pns_adapters::job_spool::Startup::Refused(refusal) =
                pns_adapters::job_spool::prepare_spool(&state)
            {
                return Err(refusal);
            }
            let tick =
                pns_application::daemon_tick(std::env::var("PNS_DAEMON_TICK_MS").ok().as_deref());
            Ok((
                pns_adapters::FileJobSpool::new(state),
                pns_adapters::DaemonChildren::new(tick),
                move || std::thread::sleep(tick),
            ))
        },
        start_retry,
        |notice| match notice {
            pns_application::DaemonNotice::Output(line) => println!("{line}"),
            pns_application::DaemonNotice::Error(line) => eprintln!("{line}"),
        },
    )
}

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
        args: vec!["daemon".into(), "retry".into()],
    })
}

pub(crate) fn daemon_retry() -> i32 {
    if std::env::args_os().nth(3).is_some() {
        eprintln!("{DAEMON_USAGE}");
        return 2;
    }
    let result = now_secs()
        .ok_or_else(|| "the clock is unavailable".to_string())
        .and_then(crate::delivery_runtime::retry_pending);
    match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("pns daemon: delivery retry failed: {error}");
            1
        }
    }
}
