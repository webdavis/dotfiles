use crate::record::{gap_line, record_detail};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use uu_application::{MarkerSnapshot, Notice, RunHeader, RunPresentation};
use uu_domain::{LaneReport, LaneVerdict};

use crate::delivery::target_name;
use crate::style::{self, Paint};
use crate::system::{host, iso};

pub struct ConsoleRunPresentation {
    log: Mutex<File>,
}

pub fn append_log(path: &Path, message: &str) {
    let result = (|| -> io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut log = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(log, "{message}")
    })();
    if let Err(error) = result {
        eprintln!("uu: could not write run log: {error}");
    }
}

impl ConsoleRunPresentation {
    pub fn new(path: &Path) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(Self {
            log: Mutex::new(OpenOptions::new().create(true).append(true).open(path)?),
        })
    }

    pub fn log_message(&self, message: &str) {
        if let Ok(mut log) = self.log.lock()
            && let Err(error) = writeln!(log, "{message}")
        {
            eprintln!("uu: could not write run log: {error}");
        }
    }
}

fn styled_record(paint: Paint, header: &RunHeader, reports: &[LaneReport]) -> String {
    let mut lines = style::header(
        paint,
        "uu run",
        &[
            style::HeaderLine {
                label: "host",
                text: &header.host,
            },
            style::HeaderLine {
                label: "started",
                text: &header.started_iso,
            },
            style::HeaderLine {
                label: "history",
                text: &header.gap,
            },
        ],
    );
    lines.push(String::new());
    lines.push(style::section(paint, "Lanes", "what this run updated"));

    let mut failures = 0;
    let mut deferred = 0;
    let mut pending = 0;
    if reports.is_empty() {
        lines.push(style::row(
            paint,
            style::Tone::Warn,
            "no lane is enabled in this config",
        ));
    }
    for report in reports {
        failures += report.failures();
        match report.verdict() {
            LaneVerdict::Deferred => deferred += 1,
            LaneVerdict::Pending => pending += 1,
            LaneVerdict::Completed | LaneVerdict::Failed => {}
        }
        let tone = match report.verdict() {
            LaneVerdict::Completed => style::Tone::Good,
            LaneVerdict::Pending | LaneVerdict::Deferred => style::Tone::Warn,
            LaneVerdict::Failed => style::Tone::Bad,
        };
        let verdict = match report.verdict() {
            LaneVerdict::Completed => "completed",
            LaneVerdict::Pending => "pending",
            LaneVerdict::Deferred => "deferred",
            LaneVerdict::Failed => "failed",
        };
        lines.push(style::row(
            paint,
            tone,
            &format!("{}: {verdict}", report.name),
        ));
        lines.extend(report.lines.iter().map(|line| style::detail(paint, line)));
    }

    let result_tone = if failures > 0 {
        style::Tone::Bad
    } else if deferred > 0 || pending > 0 {
        style::Tone::Warn
    } else {
        style::Tone::Good
    };
    lines.push(String::new());
    lines.push(style::section(paint, "Result", "what the run amounted to"));
    lines.push(style::row(
        paint,
        result_tone,
        &format!("done, {failures} failure(s), {deferred} deferred, {pending} pending"),
    ));
    lines.push(style::rule(paint));
    lines.push(String::new());
    lines.join("\n")
}

fn print_notice(paint: Paint, tone: style::Tone, message: &str, stderr: bool) {
    let line = style::row(paint, tone, message);
    if stderr {
        eprintln!("{line}");
    } else {
        println!("{line}");
    }
}

impl RunPresentation for ConsoleRunPresentation {
    fn header(&self, epoch: i64, marker: &MarkerSnapshot) -> RunHeader {
        RunHeader {
            started_iso: iso(epoch),
            gap: gap_line(&marker.value, &marker.location, epoch),
            host: host(),
        }
    }

    fn write_record(&self, header: &RunHeader, reports: &[LaneReport]) -> String {
        let detail = record_detail(&header.host, &header.started_iso, &header.gap, reports);
        print!("{}", styled_record(Paint::for_stdout(), header, reports));
        self.log_message(&detail);
        detail
    }

    fn notice(&self, notice: Notice<'_>) {
        match notice {
            Notice::NoAlerts { target, summary } => {
                let lane = target_name(target);
                let message = format!(
                    "uu: no [alerts] block; `{lane}: {summary}` was logged and nothing else"
                );
                print_notice(Paint::for_stdout(), style::Tone::Quiet, &message, false);
                self.log_message(&message);
            }
            Notice::AlertFailed { target, cause } => {
                let lane = target_name(target);
                let message = format!(
                    "uu: the alert for `{lane}` did NOT reach every configured destination ({cause}); it is logged here"
                );
                print_notice(Paint::for_stdout(), style::Tone::Bad, &message, false);
                self.log_message(&message);
            }
            Notice::NoRecords => {
                let message = "uu: no [records] block; this run was logged here and nowhere else";
                print_notice(Paint::for_stdout(), style::Tone::Warn, message, false);
                self.log_message(message);
            }
            Notice::SigningFailed => {
                let message =
                    "uu: the [records] key is empty, so nothing could be signed or posted";
                print_notice(Paint::for_stdout(), style::Tone::Bad, message, false);
                self.log_message(message);
            }
            Notice::RecordPosted(description) => {
                let message = format!("uu: {description}");
                print_notice(Paint::for_stdout(), style::Tone::Good, &message, false);
                self.log_message(&message);
            }
            Notice::MarkerClockFailed(location) => {
                let message = format!(
                    "uu: this clock could not be read at the end of the run, so the successful-run \
                     timestamp at {location} was left as it was; the next entry measures its gap from the \
                     run before this one"
                );
                print_notice(Paint::for_stderr(), style::Tone::Bad, &message, true);
                self.log_message(&message);
            }
            Notice::MarkerWriteFailed(failure) => {
                let message = format!(
                    "uu: could not record the successful-run timestamp at {}: {}; the next entry \
                     will report a stale or absent gap",
                    failure.location, failure.cause
                );
                print_notice(Paint::for_stderr(), style::Tone::Bad, &message, true);
                self.log_message(&message);
            }
            Notice::StreakWriteFailed {
                kind,
                lane,
                failure,
            } => {
                let message = format!(
                    "uu: could not record lane `{lane}`'s {} streak at {}: {}",
                    match kind {
                        uu_application::StreakKind::NonSuccess => "non-success",
                        uu_application::StreakKind::Pending => "pending",
                    },
                    failure.location,
                    failure.cause
                );
                print_notice(Paint::for_stderr(), style::Tone::Bad, &message, true);
                self.log_message(&message);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_run_record_is_appended_to_the_uu_log() {
        let log = PathBuf::from(format!("/tmp/uu-presentation-log-{}", std::process::id()));
        let _ = std::fs::remove_file(&log);
        let presentation = ConsoleRunPresentation::new(&log).expect("log");
        let mut lane = LaneReport::new("skills");
        lane.noted("updated".into());
        let header = RunHeader {
            host: "fixture-host".into(),
            started_iso: "2026-09-11T20:00:00Z".into(),
            gap: "last successful run: never".into(),
        };

        presentation.write_record(&header, &[lane]);

        let contents = std::fs::read_to_string(&log).expect("recorded log");
        assert!(contents.contains("run at 2026-09-11T20:00:00Z on fixture-host"));
        assert!(contents.contains("skills: 0 failure(s)"));
        let _ = std::fs::remove_file(log);
    }

    #[test]
    fn a_run_record_uses_the_shared_report_shape() {
        let header = RunHeader {
            host: "fixture-host".into(),
            started_iso: "2026-09-11T20:00:00Z".into(),
            gap: "last successful run: never".into(),
        };
        let mut lane = LaneReport::new("skills");
        lane.noted("updated".into());

        let rendered = styled_record(Paint::Plain, &header, &[lane]);

        assert!(rendered.contains("uu run"));
        assert!(rendered.contains("◆ Lanes"));
        assert!(rendered.contains("skills: completed"));
        assert!(rendered.contains("◆ Result"));
        assert!(rendered.contains("0 failure(s)"));
    }
}
