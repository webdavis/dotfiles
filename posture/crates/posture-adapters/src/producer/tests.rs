use super::*;
use crate::{CommandIo, CommandOutput};
use posture_application::{AlarmFailed, AlertSignal, InspectionFailure};
use posture_producer_wire::{
    DeliveryOutcome, DestinationOutcome, Request, ResultEnvelope, Status, decode_request,
};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

#[derive(Default)]
struct Alarm {
    calls: Vec<(String, String)>,
    fail: bool,
}
impl IndependentAlarm for Alarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed> {
        self.calls.push((title.into(), detail.into()));
        if self.fail { Err(AlarmFailed) } else { Ok(()) }
    }
}
struct Runner {
    response: Result<CommandOutput, InspectionFailure>,
    requests: Vec<Request>,
    /// Every argument list the command was actually run with.
    arguments: Vec<Vec<OsString>>,
    matching: bool,
}
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, Path::new("/private/fixture/engine"));
        self.arguments
            .push(args.iter().map(|arg| arg.to_os_string()).collect());
        let CommandIo::Input(input) = io else {
            panic!("request must be stdin only")
        };
        let request = decode_request(input).expect("one bounded request").request;
        let output = match &self.response {
            Ok(output) => {
                let mut bytes = output.bytes.clone();
                if self.matching {
                    let mut result = posture_producer_wire::decode_result(&bytes).unwrap();
                    result.request_id = Some(request.request_id.clone());
                    bytes = result.encode().unwrap().into_bytes();
                }
                Ok(CommandOutput {
                    bytes,
                    exit: output.exit,
                })
            }
            Err(error) => Err(*error),
        };
        self.requests.push(request);
        output
    }
}
fn alert() -> Alert {
    Alert {
        occurrence_id: Some("occurrence-7".into()),
        event: "page",
        signal: AlertSignal::NeedsAttention,
        severity: None,
        occurred_at: Some(1730000000),
        title: "Security finding".into(),
        detail: "line one\nline two".into(),
    }
}
fn subject(status: Status, committed: bool) -> ProducerCommand<Runner, Alarm> {
    let result = ResultEnvelope {
        request_id: None,
        status,
        decision_id: Some("17".into()),
        interaction: None,
        destinations: vec![DestinationOutcome {
            destination: Name::new("hermes").unwrap(),
            outcome: DeliveryOutcome::Failed,
            note: None,
        }],
        diagnostics: if committed {
            vec!["ledger_committed".into()]
        } else {
            vec![]
        },
    };
    ProducerCommand::new(
        Runner {
            response: Ok(CommandOutput {
                bytes: result.encode().unwrap().into_bytes(),
                exit: 0,
            }),
            requests: vec![],
            arguments: vec![],
            matching: true,
        },
        "/private/fixture/engine".into(),
        vec!["submit".to_string(), "--json".to_string()],
        Some(Name::new("assigned-route").unwrap()),
        Alarm::default(),
    )
}
#[test]
fn the_configured_arguments_reach_the_command_verbatim_and_nothing_is_added() {
    for arguments in [
        vec![],
        vec!["submit".to_string(), "--json".to_string()],
        vec!["page".to_string(), "--in=json".to_string(), "-".to_string()],
    ] {
        let mut sut = subject(Status::Accepted, true);
        sut.arguments = arguments.iter().cloned().map(OsString::from).collect();
        assert_eq!(sut.submit(&alert()), Submission::Accepted);
        assert_eq!(
            sut.runner.arguments,
            vec![arguments.iter().map(OsString::from).collect::<Vec<_>>()]
        );
    }
}
#[test]
fn only_a_matching_accepted_committed_receipt_advances_acceptance() {
    let mut sut = subject(Status::Accepted, true);
    assert_eq!(sut.submit(&alert()), Submission::Accepted);
    assert!(sut.alarm.calls.is_empty());
    assert_eq!(sut.runner.requests.len(), 1);
}
#[test]
fn rejection_degradation_and_missing_commitment_do_not_trigger_an_engine_alarm() {
    for (status, committed, failure) in [
        (Status::Rejected, true, SubmissionFailure::Refused),
        (Status::Degraded, true, SubmissionFailure::NotCommitted),
        (Status::Accepted, false, SubmissionFailure::NotCommitted),
    ] {
        let mut sut = subject(status, committed);
        assert_eq!(sut.submit(&alert()), Submission::NotAccepted(failure));
        assert!(sut.alarm.calls.is_empty());
        assert_eq!(sut.runner.requests.len(), 1);
    }
}
#[test]
fn an_accepted_receipt_for_another_or_missing_identity_cannot_advance_acceptance() {
    for id in [
        None,
        Some(posture_producer_wire::RequestId::new("different").unwrap()),
    ] {
        let mut sut = subject(Status::Accepted, true);
        sut.runner.matching = false;
        let output = sut.runner.response.as_mut().unwrap();
        let mut receipt = posture_producer_wire::decode_result(&output.bytes).unwrap();
        receipt.request_id = id;
        output.bytes = receipt.encode().unwrap().into_bytes();
        assert_eq!(
            sut.submit(&alert()),
            Submission::NotAccepted(SubmissionFailure::NotCommitted)
        );
        assert!(sut.alarm.calls.is_empty());
    }
}
mod failure;
mod request;

mod callers;
mod engine;
mod oversized;
