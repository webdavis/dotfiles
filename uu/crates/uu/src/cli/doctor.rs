//! `uu doctor`: what this config turns on, and what it cannot reach.

use uu_adapters::{
    Config, config_path, gap_line, home, marker_path, now_epoch, read_marker, resolve,
};

use uu_adapters::style::{self, HeaderLine, Paint, Tone};

/// THE PATH CAVEAT LIVES HERE, once, rather than on every lane that names a
/// program. It was the same forty-word sentence under four lanes, which is the
/// kind of repetition a reader learns to skip, taking the lane's actual verdict
/// with it.
const LANES_BLURB: &str =
    "what a weekly run updates; programs resolve on this shell's PATH, the run uses the plist's";
const DELIVERY_BLURB: &str = "where a run's results and its failures go";
const SCHEDULE_BLURB: &str = "when the run fires, and when it last finished";

pub fn doctor_mode() -> i32 {
    let Some(home) = home() else {
        return super::no_home();
    };
    let paint = Paint::for_stdout();
    let path = config_path(&home);
    let shown = path.display().to_string();
    print_lines(style::header(
        paint,
        "uu doctor",
        &[HeaderLine {
            label: "config",
            text: &shown,
        }],
    ));

    let config = match super::loaded(&path) {
        Ok(Some(config)) => config,
        Ok(None) => {
            print_lines(vec![
                style::section(paint, "Config", ""),
                style::row(paint, Tone::Warn, "no config file, so every lane is off"),
            ]);
            Config::default()
        }
        Err(code) => return code,
    };

    print_section(paint, "Lanes", LANES_BLURB, lanes(paint, &config));
    print_section(paint, "Delivery", DELIVERY_BLURB, delivery(paint, &config));
    print_section(
        paint,
        "Schedule",
        SCHEDULE_BLURB,
        schedule(paint, &config, &home),
    );
    0
}

/// A heading, its rows, and the blank line that separates it from the next.
///
/// THE BLANK LINE COMES FIRST, so the last section does not leave a trailing
/// gap before the prompt returns.
fn print_section(paint: Paint, title: &str, blurb: &str, rows: Vec<String>) {
    println!();
    println!("{}", style::section(paint, title, blurb));
    print_lines(rows);
}

fn print_lines(lines: Vec<String>) {
    for line in lines {
        println!("{line}");
    }
}

pub(super) fn lanes(paint: Paint, config: &Config) -> Vec<String> {
    if config.lanes.is_empty() {
        return vec![style::row(paint, Tone::Warn, "none declared")];
    }
    let has_webhook = config
        .records
        .as_ref()
        .is_some_and(|records| records.failure_webhook.is_some());
    let mut rows = Vec::new();
    for (name, lane) in &config.lanes {
        // THE TYPE IS ONLY WORTH PRINTING WHEN IT DIFFERS FROM THE NAME. Most
        // lanes are named after their type, and `brew: on (brew)` spends a
        // parenthesis telling the reader nothing.
        let kind = lane.type_name();
        let described = if kind == name {
            format!("{name}: on")
        } else {
            format!("{name}: on ({kind})")
        };
        rows.push(style::row(paint, Tone::Good, &described));
        if let Some(program) = lane.diagnostic_program() {
            rows.push(style::detail(
                paint,
                &describe_program(program, has_webhook),
            ));
        }
    }
    rows
}

/// Whether a command lane's program can be found, and the one case where
/// doctor cannot answer for the weekly run.
///
/// A SLASH-RELATIVE PROGRAM (`./updater`) is answered from DOCTOR'S OWN cwd,
/// wherever the operator happens to be standing; the weekly launchd job starts
/// at `/`, so `found` or `NOT FOUND` here says nothing about what that run
/// will see. An absolute path or a bare name on PATH resolves the same way in
/// both places, so only this case gets its own wording.
///
/// THE LANE IS NOT NAMED HERE any more. This prints directly under the lane's
/// own row, so repeating the name put it twice in two adjacent lines.
fn describe_program(program: &str, webhook: bool) -> String {
    if program.contains('/') && !program.starts_with('/') {
        return format!(
            "`{program}` is a RELATIVE PATH; the weekly run starts in /, so it resolves \
             differently there"
        );
    }
    match resolve(program) {
        // A PATH THAT RESOLVES TO ITSELF IS PRINTED ONCE. An absolute program
        // is its own answer, and `/opt/homebrew/bin/nvim found at
        // /opt/homebrew/bin/nvim` says one thing twice.
        Some(found) if found.as_os_str() == program => format!("{program}, found"),
        Some(found) => format!("`{program}` found at {}", found.display()),
        None if webhook => format!(
            "`{program}` NOT FOUND; every scheduled run of this lane will fail, and the failure \
             webhook is configured"
        ),
        None => format!(
            "`{program}` NOT FOUND; every scheduled run of this lane will fail, and it alerts only \
             when [alerts] is configured"
        ),
    }
}

fn delivery(paint: Paint, config: &Config) -> Vec<String> {
    let mut rows = Vec::new();
    // THE KEY IS NEVER PRINTED, only whether there is one.
    rows.push(match config.records.as_ref() {
        Some(records) => style::row(
            paint,
            Tone::Good,
            &format!("records post to {} (key set)", records.url),
        ),
        None => style::row(paint, Tone::Quiet, "records are off, no [records] block"),
    });

    let webhook = config
        .records
        .as_ref()
        .and_then(|records| records.failure_webhook.as_deref());
    rows.push(match config.alerts.as_ref() {
        Some(alerts) => match resolve(&alerts.binary) {
            Some(found) => style::row(
                paint,
                Tone::Good,
                &format!(
                    "alerts go through `{}`, found at {}",
                    alerts.binary,
                    found.display()
                ),
            ),
            None if webhook.is_some() => style::row(
                paint,
                Tone::Warn,
                &format!(
                    "alerts name `{}`, NOT FOUND; the failure webhook remains configured",
                    alerts.binary
                ),
            ),
            None => style::row(
                paint,
                Tone::Bad,
                &format!(
                    "alerts name `{}`, NOT FOUND; failures will be logged and nothing else",
                    alerts.binary
                ),
            ),
        },
        None if webhook.is_some() => style::row(
            paint,
            Tone::Warn,
            "no pns engine for alerts; using the failure webhook",
        ),
        None => style::row(paint, Tone::Quiet, "alerts are off, no [alerts] block"),
    });

    if let Some(url) = webhook {
        rows.push(style::row(
            paint,
            Tone::Good,
            &format!("alarms post to {url} (records key set)"),
        ));
    }
    rows
}

fn schedule(paint: Paint, config: &Config, home: &str) -> Vec<String> {
    let schedule = config.schedule;
    let marker_path = marker_path(home);
    vec![
        style::row(
            paint,
            Tone::Quiet,
            &format!(
                "weekday {} at {:02}:{:02}, which feeds `uu schedule render` only",
                schedule.weekday, schedule.hour, schedule.minute
            ),
        ),
        style::row(
            paint,
            Tone::Quiet,
            &gap_line(
                &read_marker(&marker_path),
                &marker_path.display().to_string(),
                now_epoch().unwrap_or(0),
            ),
        ),
    ]
}

#[cfg(test)]
#[path = "doctor/tests.rs"]
mod tests;
