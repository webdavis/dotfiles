//! `uu doctor`: what this config turns on, and what it cannot reach.

use uu_adapters::{
    Config, config_path, gap_line, home, marker_path, now_epoch, read_marker, resolve,
};

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

    for line in lane_descriptions(&config) {
        println!("{line}");
    }
    match config.records.as_ref() {
        // THE KEY IS NEVER PRINTED, only whether there is one.
        Some(records) => println!("uu: records: on, posting to {} (key set)", records.url),
        None => println!("uu: records: off, no [records] block"),
    }
    let webhook = config
        .records
        .as_ref()
        .and_then(|records| records.failure_webhook.as_deref());
    match config.alerts.as_ref() {
        Some(alerts) => {
            let reachable = match resolve(&alerts.binary) {
                Some(found) => format!("found at {}", found.display()),
                None if webhook.is_some() => {
                    "NOT FOUND; the failure webhook remains configured".to_string()
                }
                None => "NOT FOUND; failures will be logged and nothing else".to_string(),
            };
            println!("uu: alerts: on via `{}`, {reachable}", alerts.binary);
        }
        None if webhook.is_some() => {
            println!("uu: alerts: no pns engine; using the failure webhook")
        }
        None => println!("uu: alerts: off, no [alerts] block"),
    }
    if let Some(url) = webhook {
        println!("uu: failure webhook: on, posting alarms to {url} (records key set)");
    }
    let schedule = config.schedule;
    println!(
        "uu: schedule: weekday {} at {:02}:{:02} (this feeds `uu schedule render` only)",
        schedule.weekday, schedule.hour, schedule.minute
    );
    let marker_path = marker_path(&home);
    println!(
        "uu: {}",
        gap_line(
            &read_marker(&marker_path),
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
fn describe_program(name: &str, program: &str, webhook: bool) -> String {
    if program.contains('/') && !program.starts_with('/') {
        return format!(
            "uu: lane {name}: program `{program}`, RELATIVE PATH; the weekly run starts in /, so \
             this resolves differently there"
        );
    }
    let reachable = match resolve(program) {
        Some(found) => format!("found at {}", found.display()),
        None if webhook => "NOT FOUND; every scheduled run of this lane will fail; the failure webhook is configured".to_string(),
        None => "NOT FOUND; every scheduled run of this lane will fail, and it alerts only when \
                 [alerts] is configured"
            .to_string(),
    };
    format!(
        "uu: lane {name}: program `{program}`, {reachable} (doctor resolves on this shell's PATH; \
         the weekly run uses the plist's own PATH, which can differ)"
    )
}

pub(super) fn lane_descriptions(config: &Config) -> Vec<String> {
    if config.lanes.is_empty() {
        return vec!["uu: lanes: none declared".to_string()];
    }
    let mut lines = Vec::new();
    for (name, lane) in &config.lanes {
        lines.push(format!("uu: lane {name}: on ({})", lane.type_name()));
        if let Some(program) = lane.diagnostic_program() {
            lines.push(describe_program(
                name,
                program,
                config
                    .records
                    .as_ref()
                    .is_some_and(|records| records.failure_webhook.is_some()),
            ));
        }
    }
    lines
}
