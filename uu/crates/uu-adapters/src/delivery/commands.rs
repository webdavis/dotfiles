use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use uu_application::{AlarmKind, AlertTarget, ReportOutcome, RunReport};
use uu_domain::{DEFAULT_LANE_DEADLINE, LaneVerdict};
use uu_protocol::{AlertKind, LaneEntry, LaneOutcome, alert_document, report_document};

use super::target_name;
use crate::adapters::append_log;
use crate::lanes::CommandRunner;
use crate::runner::SystemRunner;
use crate::system::{iso, now_epoch};

pub(super) fn deliver_report(command: &[String], report: RunReport<'_>) -> ReportOutcome {
    let entries: Vec<LaneEntry<'_>> = report
        .lanes
        .iter()
        .map(|lane| LaneEntry {
            name: &lane.report.name,
            outcome: outcome(lane.report.verdict()),
            exit_code: lane.report.exit_code,
            duration_secs: lane.report.duration.as_secs(),
            output: lane.report.lines.join("\n"),
            last_ok: lane.last_ok.as_deref(),
        })
        .collect();
    let ended = iso(now_epoch().unwrap_or(0));
    let document = report_document(report.host, report.started_iso, &ended, &entries);
    match run_with_document("[report]", command, &document) {
        Ok(()) => ReportOutcome::Delivered,
        Err(why) => ReportOutcome::Failed(why),
    }
}

pub(super) fn deliver_alert(
    command: &[String],
    log: &Path,
    kind: AlarmKind,
    host: &str,
    target: AlertTarget<'_>,
    summary: &str,
) {
    let lane = match target {
        AlertTarget::Run => None,
        AlertTarget::Lane(lane) => Some(lane),
    };
    let document = alert_document(alert_kind(kind), lane, host, summary);
    if let Err(why) = run_with_document("[alert]", command, &document) {
        append_log(
            log,
            &format!(
                "uu: the [alert] command for `{}` failed: {why}",
                target_name(target)
            ),
        );
    }
}

fn alert_kind(kind: AlarmKind) -> AlertKind {
    match kind {
        AlarmKind::Failed => AlertKind::Failed,
        AlarmKind::Stale => AlertKind::Stale,
        AlarmKind::Pending => AlertKind::Pending,
        AlarmKind::RecordLost | AlarmKind::ReportUndelivered => AlertKind::ReportUndelivered,
    }
}

fn outcome(verdict: LaneVerdict) -> LaneOutcome {
    match verdict {
        LaneVerdict::Completed => LaneOutcome::Ok,
        LaneVerdict::Deferred => LaneOutcome::Deferred,
        LaneVerdict::Pending => LaneOutcome::Pending,
        LaneVerdict::Failed => LaneOutcome::Failed,
    }
}

fn run_with_document(label: &str, command: &[String], document: &str) -> Result<(), String> {
    let input = document_file(document)
        .map_err(|error| format!("could not hand {} its input: {error}", command[0]))?;
    let output = OpenOptions::new()
        .write(true)
        .open("/dev/null")
        .map_err(|error| format!("could not open /dev/null for {}: {error}", command[0]))?;
    let arguments: Vec<&str> = command[1..].iter().map(String::as_str).collect();
    SystemRunner::for_lane(label, DEFAULT_LANE_DEADLINE, DEFAULT_LANE_DEADLINE).run_to_file(
        &command[0],
        &arguments,
        input,
        output,
    )
}

fn document_file(document: &str) -> std::io::Result<File> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "uu-document-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    std::fs::remove_file(&path)?;
    file.write_all(document.as_bytes())?;
    file.seek(SeekFrom::Start(0))?;
    Ok(file)
}
