use posture_adapters::{
    CommandRunner, ControlProbes, LastResortBanner, Notify, PollStateFiles, PostureQuery,
    PostureTrio, SystemClock, SystemRunner, alert_sink, is_executable, read_controls,
};
use posture_application::{Clock, Poll, PollFailure, PollStateFailure};
use posture_domain::{ControlObservation, ControlsRead, LuluProfile};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

struct Configuration {
    state: PathBuf,
    controls: PathBuf,
    notify: Notify,
    alarm: PathBuf,
}

impl Configuration {
    fn from_home(home: &Path) -> Self {
        Self {
            state: home.join(".local/state/osquery-posture-state.json"),
            controls: home.join(".local/libexec/posture/controls.json"),
            notify: Notify::read(home),
            alarm: "/usr/bin/osascript".into(),
        }
    }
}

pub(super) fn run(stderr: &mut impl Write) -> u8 {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        let _ = writeln!(stderr, "posture poll: HOME is not set");
        return 1;
    };
    let query = query_path(std::env::var_os("PATH").as_deref());
    execute(
        Configuration::from_home(&home),
        PostureQuery::new(query),
        ControlProbes::current_user(
            "/Library/Objective-See/LuLu/rules.plist".into(),
            "/Library/Objective-See/LuLu/preferences.plist".into(),
        ),
        SystemRunner::per_command(Duration::from_secs(5)),
        SystemRunner::per_command(Duration::from_secs(10)),
        SystemClock.now().ok().map(|time| time.seconds),
        stderr,
    )
}

fn query_path(paths: Option<&std::ffi::OsStr>) -> PathBuf {
    paths
        .and_then(|paths| {
            std::env::split_paths(paths)
                .map(|path| path.join("osqueryi"))
                .find(|path| is_executable(path))
        })
        .unwrap_or_else(|| "/usr/local/bin/osqueryi".into())
}

fn execute(
    config: Configuration,
    mut query: PostureQuery<impl CommandRunner>,
    mut probes: ControlProbes<impl CommandRunner>,
    producer: impl CommandRunner,
    alarm: impl CommandRunner,
    occurred_at: Option<u64>,
    stderr: &mut impl Write,
) -> u8 {
    if let Err(error) = fs::create_dir_all(config.state.parent().unwrap_or(Path::new("."))) {
        let _ = writeln!(
            stderr,
            "posture poll: cannot create state directory: {error}"
        );
        return 1;
    }
    let current = query.read().unwrap_or_else(|_| PostureTrio::unreadable());
    let controls = read_controls(&config.controls, |error| {
        let _ = writeln!(
            stderr,
            "posture poll: {}: {error}",
            config.controls.display()
        );
    });
    let (readings, profile) = controls.as_ref().map_or_else(
        |_| (Vec::new(), LuluProfile::Base),
        |controls| probes.read(controls),
    );
    let declared = controls.as_deref().unwrap_or_default();
    let observations: Vec<_> = declared
        .iter()
        .zip(readings)
        .map(|(control, reading)| ControlObservation { control, reading })
        .collect();
    let controls = match &controls {
        Ok(_) => ControlsRead::Observed(&observations),
        Err(refusal) => ControlsRead::Refused(refusal),
    };
    let mut state = PollStateFiles::new(config.state.clone());
    let prior = state.read(declared, |error| {
        let _ = writeln!(stderr, "posture poll: {}: {error}", config.state.display());
    });
    let priors: Vec<_> = prior
        .as_ref()
        .map(|state| {
            state
                .controls
                .iter()
                .map(|control| control.prior())
                .collect()
        })
        .unwrap_or_default();
    let prior = prior.as_ref().and_then(|state| state.baseline(&priors));
    let mut sink = alert_sink(
        config.notify,
        producer,
        LastResortBanner::new(alarm, config.alarm),
        &mut *stderr,
    );
    let result = Poll {
        markers: &state,
        sink: &mut sink,
        publish: |update: &posture_domain::BaselineUpdate| {
            state
                .publish(update, &current)
                .map_err(|_| PollStateFailure)
        },
    }
    .run(current.reading(), controls, prior, profile, occurred_at);
    match result {
        Ok(()) => 0,
        Err(PollFailure::Gap(_)) => {
            let _ = writeln!(
                stderr,
                "firewall-gatekeeper-monitor: send_alert could not queue the monitoring-gap page; no marker written, retrying next tick"
            );
            1
        }
        Err(PollFailure::Exposure(_)) => {
            let _ = writeln!(
                stderr,
                "firewall-gatekeeper-monitor: send_alert could not queue the CRIT page; baseline not advanced, retrying next tick"
            );
            1
        }
        Err(PollFailure::Persistence) => 1,
    }
}

#[cfg(test)]
mod tests;
