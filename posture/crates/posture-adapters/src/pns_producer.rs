use crate::{CommandIo, CommandRunner};
use posture_application::{
    Alert, AlertSignal, AlertSink, IndependentAlarm, InspectionFailure, Submission,
    SubmissionFailure,
};
use posture_pns_wire::{Name, RequestId, Status, decode_result};
use std::{ffi::OsStr, path::PathBuf};
mod request;

pub struct PnsProducer<R, A> {
    runner: R,
    executable: PathBuf,
    route: Option<Name>,
    alarm: A,
}
impl<R: CommandRunner, A: IndependentAlarm> PnsProducer<R, A> {
    pub fn new(runner: R, executable: PathBuf, route: Option<Name>, alarm: A) -> Self {
        Self {
            runner,
            executable,
            route,
            alarm,
        }
    }
    fn failed_engine(&mut self, alert: &Alert, failure: SubmissionFailure) -> Submission {
        let _ = self.alarm.alarm(
            "Posture notification engine failed",
            &format!("{}\n{}", alert.title, alert.detail),
        );
        Submission::NotAccepted(failure)
    }
    fn send(&mut self, alert: &Alert, (identity, input): (RequestId, String)) -> Submission {
        let output = match self.runner.run_completed(
            &self.executable,
            &[OsStr::new("submit"), OsStr::new("--json")],
            CommandIo::Input(input.as_bytes()),
        ) {
            Ok(output) => output,
            Err(error) => {
                return self.failed_engine(
                    alert,
                    match error {
                        InspectionFailure::Unavailable => SubmissionFailure::Unavailable,
                        InspectionFailure::Failed => SubmissionFailure::Failed,
                        InspectionFailure::TimedOut => SubmissionFailure::TimedOut,
                    },
                );
            }
        };
        let Ok(result) = decode_result(&output.bytes) else {
            return self.failed_engine(
                alert,
                if output.exit == 0 {
                    SubmissionFailure::Unparseable
                } else {
                    SubmissionFailure::Failed
                },
            );
        };
        if result.request_id.as_ref() != Some(&identity) {
            return if output.exit == 0 {
                Submission::NotAccepted(SubmissionFailure::NotCommitted)
            } else {
                self.failed_engine(alert, SubmissionFailure::Failed)
            };
        }
        // pns returns exit 2 for a normal protocol refusal. Its correlated result owns that meaning.
        if result.status == Status::Rejected {
            if result
                .diagnostics
                .iter()
                .any(|code| code == "submission_unavailable")
            {
                return self.failed_engine(alert, SubmissionFailure::Unavailable);
            }
            return Submission::NotAccepted(SubmissionFailure::Refused);
        }
        if output.exit != 0 {
            return self.failed_engine(alert, SubmissionFailure::Failed);
        }
        match result.status {
            Status::Accepted
                if result
                    .diagnostics
                    .iter()
                    .any(|code| code == "ledger_committed") =>
            {
                Submission::Accepted
            }
            _ => Submission::NotAccepted(SubmissionFailure::NotCommitted),
        }
    }
}
impl<R: CommandRunner, A: IndependentAlarm> AlertSink for PnsProducer<R, A> {
    fn submit(&mut self, alert: &Alert) -> Submission {
        match request::encode(alert, self.route.clone()) {
            Ok(request) => return self.send(alert, request),
            Err(request::EncodeFailure::Oversized)
                if alert.signal == AlertSignal::NeedsAttention =>
            {
                let notice = request::omission(alert);
                if let Ok(request) = request::encode(&notice, self.route.clone()) {
                    let _ = self.send(&notice, request);
                }
                // Acceptance of this bounded notice never acknowledges the omitted finding.
            }
            Err(_) => {}
        }
        Submission::NotAccepted(SubmissionFailure::Refused)
    }
}
#[cfg(test)]
mod tests;
