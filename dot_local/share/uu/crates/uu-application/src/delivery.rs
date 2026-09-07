use crate::ports::{
    AlarmKind, AlertOutcome, AlertTarget, Notice, RecordOutcome, RunDelivery, RunPresentation,
    RunRecord,
};

/// No configured engine means nothing is owed. A configured failure stays owed
/// so a one-shot staleness alert can retry on the next run.
pub(crate) fn send_alert(
    delivery: &impl RunDelivery,
    presentation: &impl RunPresentation,
    kind: AlarmKind,
    host: &str,
    target: AlertTarget<'_>,
    summary: &str,
) -> AlertOutcome {
    let outcome = delivery.alert(kind, host, target, summary);
    match &outcome {
        AlertOutcome::NotConfigured => presentation.notice(Notice::NoAlerts { target, summary }),
        AlertOutcome::Failed(cause) => presentation.notice(Notice::AlertFailed { target, cause }),
        AlertOutcome::Delivered => {}
    }
    outcome
}

/// A refused record is printed and alerted. No configured record channel owes
/// nothing, while a key that could not sign leaves the marker unchanged.
pub(crate) fn deliver_record(
    delivery: &impl RunDelivery,
    presentation: &impl RunPresentation,
    record: RunRecord<'_>,
) -> bool {
    let host = record.host;
    match delivery.record(record) {
        RecordOutcome::NotConfigured => {
            presentation.notice(Notice::NoRecords);
            true
        }
        RecordOutcome::SigningFailed => {
            presentation.notice(Notice::SigningFailed);
            false
        }
        RecordOutcome::Delivered { description } => {
            presentation.notice(Notice::RecordPosted(&description));
            true
        }
        RecordOutcome::Rejected {
            url, description, ..
        } => {
            presentation.notice(Notice::RecordPosted(&description));
            send_alert(
                delivery,
                presentation,
                AlarmKind::RecordLost,
                host,
                AlertTarget::Run,
                &format!(
                    "the weekly record could NOT be delivered to {url} ({description}); until this is fixed that \
                     channel is silent for a reason that has nothing to do with the jobs it reports on"
                ),
            );
            false
        }
    }
}

#[cfg(test)]
mod tests;
