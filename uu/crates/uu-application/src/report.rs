use uu_domain::{LaneReport, LaneVerdict, Marker};

use crate::delivery::send_alert;
use crate::ports::{
    AlarmKind, AlertTarget, Notice, ReportOutcome, ReportedLane, RunDelivery, RunHeader,
    RunPresentation, RunReport, RunState,
};

pub(crate) fn deliver_report(
    state: &impl RunState,
    delivery: &impl RunDelivery,
    presentation: &impl RunPresentation,
    header: &RunHeader,
    started: Option<i64>,
    reports: &[LaneReport],
) {
    let lanes: Vec<ReportedLane<'_>> = reports
        .iter()
        .map(|report| ReportedLane {
            report,
            last_ok: last_ok(state, presentation, header, started, report),
        })
        .collect();
    let outcome = delivery.report(RunReport {
        host: &header.host,
        started_iso: &header.started_iso,
        lanes: &lanes,
    });
    if let ReportOutcome::Failed(why) = outcome {
        send_alert(
            delivery,
            presentation,
            AlarmKind::ReportUndelivered,
            &header.host,
            AlertTarget::Run,
            &format!("the run report was NOT delivered: {why}"),
        );
    }
}

fn last_ok(
    state: &impl RunState,
    presentation: &impl RunPresentation,
    header: &RunHeader,
    started: Option<i64>,
    report: &LaneReport,
) -> Option<String> {
    match (report.verdict(), started) {
        (LaneVerdict::Completed, Some(epoch)) => {
            if let Err(failure) = state.write_last_ok(&report.name, epoch) {
                presentation.notice(Notice::MarkerWriteFailed(&failure));
            }
            Some(header.started_iso.clone())
        }
        _ => match state.last_ok(&report.name) {
            Marker::Recorded { iso, .. } => Some(iso),
            Marker::NeverRecorded | Marker::Unreadable => None,
        },
    }
}
