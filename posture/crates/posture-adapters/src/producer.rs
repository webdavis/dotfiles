//! The producer path: posture hands one page to the command the operator
//! configured and reads that command's answer back.
//!
//! THE PRODUCER API IS THE WHOLE COUPLING. A JSON request goes in on standard
//! input, a JSON result plus an exit code comes back, and the command and its
//! arguments are both config. posture therefore names no engine: any program
//! that serves the contract serves posture, and which one does is a
//! per-machine choice.

use crate::sink::{delivery_failed, tier_route};
use crate::wire::{Name, RequestId, Status, decode_result};
use crate::{CommandIo, CommandRunner};
use posture_application::{
    Alert, AlertSignal, AlertSink, IndependentAlarm, InspectionFailure, Submission,
    SubmissionFailure,
};
use std::{ffi::OsStr, ffi::OsString, path::PathBuf};
mod request;

/// The title the local banner carries when the producer command itself broke.
const ALARM_TITLE: &str = "Posture notification engine failed";

pub struct ProducerCommand<R, A> {
    runner: R,
    executable: PathBuf,
    /// The command's arguments, passed verbatim. The contract says nothing
    /// about them: an engine may spell its own submit path any way it likes.
    arguments: Vec<OsString>,
    route: Option<Name>,
    alarm: A,
}
impl<R: CommandRunner, A: IndependentAlarm> ProducerCommand<R, A> {
    pub fn new(
        runner: R,
        executable: PathBuf,
        arguments: Vec<String>,
        route: Option<Name>,
        alarm: A,
    ) -> Self {
        Self {
            runner,
            executable,
            arguments: arguments.into_iter().map(OsString::from).collect(),
            route,
            alarm,
        }
    }
    /// The route this alert belongs on: its tier's, when it has one, and
    /// otherwise the one this producer was built with.
    fn route_for(&self, alert: &Alert) -> Option<Name> {
        tier_route(alert).or_else(|| self.route.clone())
    }
    fn failed_engine(&mut self, alert: &Alert, failure: SubmissionFailure) -> Submission {
        delivery_failed(&mut self.alarm, ALARM_TITLE, alert, failure)
    }
    fn send(&mut self, alert: &Alert, (identity, input): (RequestId, String)) -> Submission {
        let arguments: Vec<&OsStr> = self.arguments.iter().map(OsString::as_os_str).collect();
        let output = match self.runner.run_completed(
            &self.executable,
            &arguments,
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
        // An engine returns exit 2 for a normal protocol refusal. Its correlated result owns that meaning.
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
        // Delivered, Partial and Undelivered all mean the engine took durable
        // ownership of the request; only the diagnostic proves it committed.
        // A non-zero exit here (pns exits 1 for Partial/Undelivered) reports
        // the destination outcome, not a broken engine.
        match result.status {
            Status::Delivered | Status::Partial | Status::Undelivered
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
impl<R: CommandRunner, A: IndependentAlarm> AlertSink for ProducerCommand<R, A> {
    fn submit(&mut self, alert: &Alert) -> Submission {
        let route = self.route_for(alert);
        match request::encode(alert, route.clone()) {
            Ok(request) => return self.send(alert, request),
            Err(request::EncodeFailure::Oversized)
                if alert.signal == AlertSignal::NeedsAttention =>
            {
                let notice = request::omission(alert);
                if let Ok(request) = request::encode(&notice, route) {
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
