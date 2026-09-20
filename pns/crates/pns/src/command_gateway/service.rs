use super::GATEWAY_USAGE;
use pns_application::{ServiceController, ServiceError, ServiceState};

/// The four launchd verbs of `pns gateway`, wrapped the way `hermes gateway`
/// wraps its own, and the refusal every word this subcommand does not serve
/// ends at.
pub(super) fn service_mode(verb: &str) -> i32 {
    // Filtered here, before HOME or the config is touched: an unknown verb
    // reads and spawns nothing, matching the forbidden-side-effect contract.
    if !matches!(verb, "start" | "stop" | "restart" | "status") {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let controller = pns_adapters::LaunchdServiceController {
        runner: pns_adapters::SystemLaunchctlRunner,
        home: home.clone(),
    };
    run_gateway(
        verb,
        std::env::consts::OS,
        service_label(&home).as_deref(),
        &controller,
    )
}

/// The decision layer, free of the real OS, the real config and the real
/// launchd: which verb runs, what it prints, and every exit code, all read
/// off `os`, `label` and a `ServiceController`, so the whole ladder is tested
/// against plain values and a scripted double.
fn run_gateway(
    verb: &str,
    os: &str,
    label: Option<&str>,
    controller: &impl ServiceController,
) -> i32 {
    if !matches!(verb, "start" | "stop" | "restart" | "status") {
        eprintln!("{GATEWAY_USAGE}");
        return 2;
    }
    // LAUNCHD ONLY, checked before the label even matters: a Linux machine has
    // no service to name, so failing here costs it nothing it would have had.
    if os != "macos" {
        eprintln!("pns gateway: launchd only");
        return 2;
    }
    let Some(label) = label else {
        eprintln!("pns gateway: `[gateway] service` is not set");
        return 2;
    };
    match verb {
        "start" => start(label, controller),
        "stop" => stop(label, controller),
        "restart" => restart(label, controller),
        "status" => status(label, controller),
        _ => unreachable!("filtered above"),
    }
}

/// `[gateway] service`, or `None` for an absent key, an absent config, or one
/// that will not load: every one of those is a machine that told this verb
/// nothing to act on.
fn service_label(home: &str) -> Option<String> {
    match pns_adapters::load_config(&pns_adapters::config_path(home)) {
        Ok(pns_adapters::LoadOutcome::Loaded(config)) => config.gateway_service,
        _ => None,
    }
}

fn start(label: &str, controller: &impl ServiceController) -> i32 {
    match controller.start(label) {
        Ok(()) => {
            println!("pns gateway: started");
            0
        }
        Err(error) => report_failure("start", error),
    }
}

fn stop(label: &str, controller: &impl ServiceController) -> i32 {
    match controller.stop(label) {
        Ok(()) => {
            println!("pns gateway: stopped");
            0
        }
        // NOT AN ERROR: the end state the operator asked for (the service
        // gone) is the one they already have.
        Err(ServiceError::NotLoaded) => {
            println!("not loaded");
            0
        }
        Err(error) => report_failure("stop", error),
    }
}

/// `restart` FALLS BACK TO `start` on a service nothing has bootstrapped,
/// rather than reporting `not loaded` the way `stop` does: an operator
/// restarting a service that turns out to be down wants it running, and
/// `start` is the whole of what running it takes.
fn restart(label: &str, controller: &impl ServiceController) -> i32 {
    match controller.restart(label) {
        Ok(()) => {
            println!("pns gateway: restarted");
            0
        }
        Err(ServiceError::NotLoaded) => start(label, controller),
        Err(error) => report_failure("restart", error),
    }
}

fn status(label: &str, controller: &impl ServiceController) -> i32 {
    match controller.status(label) {
        Ok(ServiceState::Running { pid }) => {
            println!("running (pid {pid})");
            0
        }
        Ok(ServiceState::Loaded) => {
            println!("loaded, not running");
            0
        }
        // EXIT 1 AND NOT 2: the command was well formed and answered; the
        // answer is just one a script testing for the service can act on.
        Ok(ServiceState::NotLoaded) => {
            println!("not loaded");
            1
        }
        Err(error) => report_failure("status", error),
    }
}

/// launchctl's own refusal, or the missing plist `start` refuses before
/// running launchctl at all.
///
/// `NotLoaded` NEVER REACHES HERE: `stop` and `restart`, the two verbs that
/// can produce it, each read it before falling through to this report, and
/// `status` never produces it at all. The arm below is a defensive answer
/// for a case the command layer above already owns.
fn report_failure(verb: &str, error: ServiceError) -> i32 {
    match error {
        ServiceError::MissingPlist(path) => {
            eprintln!("pns gateway: {path} is missing; a chezmoi apply installs it");
            2
        }
        ServiceError::NotLoaded => {
            eprintln!("pns gateway {verb}: not loaded");
            1
        }
        ServiceError::Failed(message) => {
            eprintln!("pns gateway {verb}: {message}");
            1
        }
    }
}

#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
