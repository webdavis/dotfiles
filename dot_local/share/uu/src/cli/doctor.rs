//! `uu doctor`: what this config turns on, and what it cannot reach.

use unattended_upgrades::config::{Config, LaneKind, config_path};
use unattended_upgrades::record::gap_line;

use crate::state::marker;
use crate::system::{home, now_epoch, resolve};

pub fn doctor_mode() -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };
    let path = config_path(&home);
    println!("uu: config {}", path.display());
    let config = match super::loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("uu: no config file; every lane is off");
            Config::default()
        }
        Err(code) => return code,
    };

    if config.lanes.is_empty() {
        println!("uu: lanes: none declared");
    } else {
        for (name, lane) in &config.lanes {
            println!("uu: lane {name}: on ({})", lane.kind.type_name());
            if let LaneKind::Command(command) = &lane.kind {
                report_program(name, &command.run[0]);
            }
        }
    }
    match config.records.as_ref() {
        // THE KEY IS NEVER PRINTED, only whether there is one.
        Some(records) => println!("uu: records: on, posting to {} (key set)", records.url),
        None => println!("uu: records: off, no [records] block"),
    }
    match config.alerts.as_ref() {
        Some(alerts) => {
            let reachable = match resolve(&alerts.binary) {
                Some(found) => format!("found at {}", found.display()),
                None => "NOT FOUND; failures will be logged and nothing else".to_string(),
            };
            println!("uu: alerts: on via `{}`, {reachable}", alerts.binary);
        }
        None => println!("uu: alerts: off, no [alerts] block"),
    }
    let schedule = config.schedule;
    println!(
        "uu: schedule: weekday {} at {:02}:{:02} (this feeds `uu schedule render` only)",
        schedule.weekday, schedule.hour, schedule.minute
    );
    let marker_path = marker::path(&home);
    println!(
        "uu: {}",
        gap_line(
            &marker::read(&marker_path),
            &marker_path.display().to_string(),
            now_epoch().unwrap_or(0)
        )
    );
    0
}

/// Whether a command lane's program can be found, and the one case where
/// doctor cannot answer for the weekly run.
///
/// A SLASH-RELATIVE PROGRAM (`./updater`) is answered from DOCTOR'S OWN cwd,
/// wherever the operator happens to be standing; the weekly launchd job starts
/// at `/`, so `found` or `NOT FOUND` here says nothing about what that run
/// will see. An absolute path or a bare name on PATH resolves the same way in
/// both places, so only this case gets its own line instead of a resolution.
fn report_program(name: &str, program: &str) {
    if program.contains('/') && !program.starts_with('/') {
        println!(
            "uu: lane {name}: program `{program}`, RELATIVE PATH; the weekly run starts in /, so \
             this resolves differently there"
        );
        return;
    }
    let reachable = match resolve(program) {
        Some(found) => format!("found at {}", found.display()),
        None => "NOT FOUND; every scheduled run of this lane will fail, and it alerts only when \
                 [alerts] is configured"
            .to_string(),
    };
    println!(
        "uu: lane {name}: program `{program}`, {reachable} (doctor resolves on this shell's PATH; \
         the weekly run uses the plist's own PATH, which can differ)"
    );
}
