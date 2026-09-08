use crate::*;

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
        |notice| match notice {
            pns_application::DaemonNotice::Output(line) => println!("{line}"),
            pns_application::DaemonNotice::Error(line) => eprintln!("{line}"),
        },
    )
}
