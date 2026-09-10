use super::*;
use crate::{CommandIo, CommandOutput};
use posture_pns_wire::{
    DeliveryOutcome, DestinationOutcome, Request, ResultEnvelope, Status, decode_request,
};
use posture_application::{AlarmFailed, AlertSignal, InspectionFailure};
use std::{ffi::OsStr, path::Path};

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
    matching: bool,
}
impl CommandRunner for Runner {
    fn run_completed(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        io: CommandIo<'_>,
    ) -> Result<CommandOutput, InspectionFailure> {
        assert_eq!(program, Path::new("/private/fixture/pns"));
        assert_eq!(args, [OsStr::new("submit"), OsStr::new("--json")]);
        let CommandIo::Input(input) = io else {
            panic!("request must be stdin only")
        };
        let request = decode_request(input).expect("one bounded request").request;
        let output = match &self.response {
            Ok(output) => {
                let mut bytes = output.bytes.clone();
                if self.matching {
                    let mut result = posture_pns_wire::decode_result(&bytes).unwrap();
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
        occurred_at: Some(1730000000),
        title: "Security finding".into(),
        detail: "line one\nline two".into(),
    }
}
fn subject(status: Status, committed: bool) -> PnsProducer<Runner, Alarm> {
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
    PnsProducer::new(
        Runner {
            response: Ok(CommandOutput {
                bytes: result.encode().unwrap().into_bytes(),
                exit: 0,
            }),
            requests: vec![],
            matching: true,
        },
        "/private/fixture/pns".into(),
        Some(Name::new("assigned-route").unwrap()),
        Alarm::default(),
    )
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
        Some(posture_pns_wire::RequestId::new("different").unwrap()),
    ] {
        let mut sut = subject(Status::Accepted, true);
        sut.runner.matching = false;
        let output = sut.runner.response.as_mut().unwrap();
        let mut receipt = posture_pns_wire::decode_result(&output.bytes).unwrap();
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

mod engine;
