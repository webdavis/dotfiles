use crate::record::{gap_line, record_detail};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::Mutex;
use uu_application::{MarkerSnapshot, Notice, RunHeader, RunPresentation};
use uu_domain::LaneReport;

use crate::delivery::target_name;
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
        print!("{detail}");
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
                println!("{message}");
                self.log_message(&message);
            }
            Notice::AlertFailed { target, cause } => {
                let lane = target_name(target);
                let message = format!(
                    "uu: the alert for `{lane}` did NOT reach every configured destination ({cause}); it is logged here"
                );
                println!("{message}");
                self.log_message(&message);
            }
            Notice::NoRecords => {
                let message = "uu: no [records] block; this run was logged here and nowhere else";
                println!("{message}");
                self.log_message(message);
            }
            Notice::SigningFailed => {
                let message =
                    "uu: the [records] key is empty, so nothing could be signed or posted";
                println!("{message}");
                self.log_message(message);
            }
            Notice::RecordPosted(description) => {
                let message = format!("uu: {description}");
                println!("{message}");
                self.log_message(&message);
            }
            Notice::MarkerClockFailed(location) => {
                let message = format!(
                    "uu: this clock could not be read at the end of the run, so the successful-run \
                     timestamp at {location} was left as it was; the next entry measures its gap from the \
                     run before this one"
                );
                eprintln!("{message}");
                self.log_message(&message);
            }
            Notice::MarkerWriteFailed(failure) => {
                let message = format!(
                    "uu: could not record the successful-run timestamp at {}: {}; the next entry \
                     will report a stale or absent gap",
                    failure.location, failure.cause
                );
                eprintln!("{message}");
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
                eprintln!("{message}");
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
}
