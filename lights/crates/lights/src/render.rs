use crate::Response;
use lights_application::{LightsError, NoPresetNow};
use lights_domain::{Action, Presets};

pub(super) fn error(error: LightsError) -> (u8, String) {
    match error {
        LightsError::UnknownRoom { name } => (2, format!("unknown room {name:?}")),
        LightsError::UnknownScene { name, room } => {
            (3, format!("unknown scene {name:?} in room {room:?}"))
        }
        LightsError::Unreachable { detail }
        | LightsError::Refused { detail }
        | LightsError::Malformed { detail } => (4, detail),
        LightsError::InvalidReference => (4, "invalid controller reference".into()),
    }
}

const NO_WINDOWS: &str = "no preset windows configured; add a [[preset_windows]] entry \
                          with start, end and preset";

/// Every arm names what the operator has to add or fix, because a bare
/// "nothing happened" from a key press is indistinguishable from a dead key.
pub(super) fn no_preset_now(refusal: NoPresetNow) -> String {
    match refusal {
        NoPresetNow::NoWindows => NO_WINDOWS.into(),
        NoPresetNow::ClockUnavailable => "cannot read the local clock".into(),
        NoPresetNow::Uncovered(minute) => format!(
            "no preset window covers {:02}:{:02}",
            minute / 60,
            minute % 60
        ),
    }
}

pub(super) fn preset_names(presets: &Presets) -> String {
    presets.names().map(|name| format!("{name}\n")).collect()
}

/// One line per room, successes on stdout and failures on stderr, and the exit
/// code of the FIRST failure. A later room's fault does not relabel an earlier
/// one, and a run that lit every room it could still exits non-zero. Serves a
/// preset's plan and `--all` alike: both are a list of rooms to walk.
pub(super) fn per_room(results: &[Result<Action, LightsError>]) -> Response {
    let mut response = Response {
        exit: 0,
        stdout: String::new(),
        stderr: String::new(),
    };
    for result in results {
        match result {
            Ok(action) => response
                .stdout
                .push_str(&lights_adapters::render_action(action)),
            Err(failed) => {
                let (exit, message) = error(failed.clone());
                if response.exit == 0 {
                    response.exit = exit;
                }
                response.stderr.push_str(&format!("lights: {message}\n"));
            }
        }
    }
    response
}
