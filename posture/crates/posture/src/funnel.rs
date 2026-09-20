mod configuration;
use configuration::Configuration;
use posture_adapters::{
    FunnelStateFile, LastResortBanner, SystemClock, SystemRunner, alert_sink, read_funnel,
};
use posture_application::{Clock, Funnel, FunnelFailure};
use posture_domain::FunnelReadFailure;
use std::{io::Write, time::Duration};

pub(super) fn run(stderr: &mut impl Write) -> u8 {
    let Some(config) = Configuration::read(|name| std::env::var_os(name)) else {
        let _ = stderr.write_all(b"posture funnel: HOME is not set\n");
        return 1;
    };
    if let Some(parent) = config.state.parent()
        && std::fs::create_dir_all(parent).is_err()
    {
        let _ = stderr.write_all(b"posture funnel: cannot create the state directory\n");
        return 1;
    }
    let store = FunnelStateFile::new(config.state);
    let baseline = store.read();
    let reading = match config.budget {
        Some(budget) => read_funnel(&mut SystemRunner::per_command(budget), &config.tailscale),
        None => Err(FunnelReadFailure::Status(125)),
    };
    if let Err(FunnelReadFailure::MissingBinary(path)) = &reading {
        let _ = writeln!(
            stderr,
            "WARN: no tailscale binary ({path}) - funnel monitoring is blind"
        );
    }
    let mut sink = alert_sink(
        config.notify,
        SystemRunner::per_command(Duration::from_secs(5)),
        LastResortBanner::new(
            SystemRunner::per_command(Duration::from_secs(10)),
            "/usr/bin/osascript".into(),
        ),
        &mut *stderr,
    );
    let now = SystemClock.now().ok().map(|time| time.seconds);
    let result = Funnel {
        store: &store,
        sink: &mut sink,
    }
    .run(reading, baseline, now);
    let message = match result {
        Ok(()) => return 0,
        Err(FunnelFailure::ReadGap(_)) => {
            "posture funnel: could not queue the monitoring-gap page; no marker written, retrying next tick\n"
        }
        Err(FunnelFailure::Exposure(_)) => {
            "posture funnel: could not queue the funnel-exposure page; baseline not advanced, retrying next tick\n"
        }
        Err(FunnelFailure::CorruptBaseline(_)) => {
            "posture funnel: could not queue the corrupt-baseline warning; the baseline was repaired anyway\n"
        }
        Err(FunnelFailure::Persistence) => "posture funnel: could not persist the baseline\n",
    };
    let _ = stderr.write_all(message.as_bytes());
    1
}
