//! Load run configuration and compose the application with concrete adapters.

use uu_adapters::{Config, config_path};
use uu_application::{LockFailure, Run, RunClock, RunOutcome, RunPresentation, RunRequest};

use uu_adapters::home;
use uu_adapters::{
    ConfiguredLaneExecutor, ConsoleRunPresentation, EngineRunDelivery, FileRunState,
    SystemRunClock, append_log, log_path,
};

pub fn run_mode(only: Option<&str>) -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };

    let path = config_path(&home);
    let config = match super::loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            // A bare run on a configless machine is clean by design. A lane
            // asked for BY NAME is a request, and one no file declares did
            // not run, so it is refused the way an undeclared name is below.
            if let Some(lane) = only {
                eprintln!(
                    "uu: no config at {}, so no lane `{lane}` is declared",
                    path.display()
                );
                return 1;
            }
            println!(
                "uu: no config at {}; nothing is enabled and nothing was updated",
                path.display()
            );
            return 0;
        }
        Err(code) => return code,
    };

    let log = log_path(&home);
    let presentation = match ConsoleRunPresentation::new(&log) {
        Ok(presentation) => presentation,
        Err(error) => {
            eprintln!("uu: could not open run log {}: {error}", log.display());
            return 1;
        }
    };
    match execute(&home, &config, only, SystemRunClock, presentation) {
        RunOutcome::Completed => 0,
        RunOutcome::UndeclaredLane => {
            if let Some(lane) = only {
                eprintln!(
                    "uu: lane `{lane}` has no `[lanes.{lane}]` block in {}",
                    path.display()
                );
            }
            1
        }
        RunOutcome::LockRefused(LockFailure::Contended(why)) => {
            let message =
                format!("uu: {why}; not running, to avoid racing the run that already holds it");
            eprintln!("{message}");
            append_log(&log, &message);
            1
        }
        RunOutcome::LockRefused(LockFailure::Unavailable(why)) => {
            let message = format!("uu: {why}; not running");
            eprintln!("{message}");
            append_log(&log, &message);
            1
        }
    }
}

fn execute(
    home: &str,
    config: &Config,
    only: Option<&str>,
    clock: impl RunClock,
    presentation: impl RunPresentation,
) -> RunOutcome {
    let lanes = config
        .lanes
        .iter()
        .map(|(name, lane)| {
            (
                name.clone(),
                uu_application::LaneSettings {
                    deadline: lane.deadline,
                    escalate_after_runs: lane.escalate_after_runs,
                },
            )
        })
        .collect();
    let run = Run {
        state: FileRunState(home),
        clock,
        lanes: ConfiguredLaneExecutor(config),
        delivery: EngineRunDelivery::new(
            config.records.as_ref(),
            config.alerts.as_ref().map(|alerts| alerts.binary.as_str()),
        ),
        presentation,
    };
    run.execute(RunRequest {
        lanes: &lanes,
        only,
    })
}

#[cfg(test)]
mod tests;
