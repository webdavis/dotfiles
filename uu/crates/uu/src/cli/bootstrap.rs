use uu_adapters::style::{self, Paint, Tone};
use uu_adapters::{FileRunState, bootstrap_lane, config_path, home};
use uu_application::{BootstrapOutcome, LockFailure, bootstrap};
use uu_domain::LaneVerdict;

pub fn bootstrap_mode(lane: &str) -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };
    let path = config_path(&home);
    let config = match super::loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            eprintln!(
                "{}",
                style::row(
                    Paint::for_stderr(),
                    Tone::Bad,
                    &format!(
                        "uu: no config at {}, so no lane `{lane}` is declared",
                        path.display()
                    ),
                )
            );
            return 1;
        }
        Err(code) => return code,
    };
    match bootstrap(&FileRunState(&home), || {
        bootstrap_lane(&home, &config, lane)
    }) {
        BootstrapOutcome::Reported(report) => {
            for line in &report.lines {
                println!("{}", style::detail(Paint::for_stdout(), line));
            }
            i32::from(report.verdict() != LaneVerdict::Completed)
        }
        BootstrapOutcome::Undeclared => {
            eprintln!(
                "{}",
                style::row(
                    Paint::for_stderr(),
                    Tone::Bad,
                    &format!(
                        "uu: lane `{lane}` has no `[lanes.{lane}]` block in {}",
                        path.display()
                    ),
                )
            );
            1
        }
        BootstrapOutcome::Unsupported(kind) => {
            eprintln!(
                "{}",
                style::row(
                    Paint::for_stderr(),
                    Tone::Bad,
                    &format!("uu: lane `{lane}` of type `{kind}` has no bootstrap step"),
                )
            );
            1
        }
        BootstrapOutcome::LockRefused(LockFailure::Contended(why)) => {
            eprintln!(
                "{}",
                style::row(
                    Paint::for_stderr(),
                    Tone::Bad,
                    &format!(
                        "uu: {why}; not bootstrapping, to avoid racing the run that already holds it"
                    ),
                )
            );
            1
        }
        BootstrapOutcome::LockRefused(LockFailure::Unavailable(why)) => {
            eprintln!(
                "{}",
                style::row(
                    Paint::for_stderr(),
                    Tone::Bad,
                    &format!("uu: {why}; not bootstrapping")
                ),
            );
            1
        }
    }
}
