//! Load run configuration and compose the application with concrete adapters.

use uu_adapters::config_path;
use uu_application::{LockFailure, Run, RunOutcome, RunRequest};

use uu_adapters::home;
use uu_adapters::{
    ConfiguredLaneExecutor, ConsoleRunPresentation, EngineRunDelivery, FileRunState, SystemRunClock,
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

    let lanes = config
        .lanes
        .iter()
        .map(|(name, lane)| (name.clone(), lane.deadline))
        .collect();
    let run = Run {
        state: FileRunState(&home),
        clock: SystemRunClock,
        lanes: ConfiguredLaneExecutor(&config),
        delivery: EngineRunDelivery::new(
            config.records.as_ref(),
            config.alerts.as_ref().map(|alerts| alerts.binary.as_str()),
        ),
        presentation: ConsoleRunPresentation,
    };
    match run.execute(RunRequest {
        lanes: &lanes,
        only,
    }) {
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
            eprintln!("uu: {why}; not running, to avoid racing the run that already holds it");
            1
        }
        RunOutcome::LockRefused(LockFailure::Unavailable(why)) => {
            eprintln!("uu: {why}; not running");
            1
        }
    }
}
