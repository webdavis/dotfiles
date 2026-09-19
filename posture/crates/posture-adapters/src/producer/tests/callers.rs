use super::*;
use posture_application::{
    BatchPage, CursorStore, JudgeFindings, JudgeOutcome, JudgeResults, JudgedBatch, Poll,
    PollFailure, PollGap, PollMarkers, PollStateFailure, ResultsLog, RunLock,
};
use posture_domain::{
    ControlRecord, ControlsInput, ControlsRead, LiveLog, LuluProfile, StoredCursor, TrioReading,
    validate_controls,
};

struct Markers;
impl PollMarkers for Markers {
    fn covered(&self, _: PollGap) -> Vec<String> {
        vec![]
    }
    fn remember(&self, _: PollGap, _: &[String]) -> Result<(), PollStateFailure> {
        panic!("an omission must not acknowledge a monitoring gap")
    }
    fn clear(&self, _: PollGap) -> Result<(), PollStateFailure> {
        panic!("unreadable controls cannot clear the reading gap")
    }
}

#[test]
fn an_oversized_poll_gap_reports_omission_without_marking_or_publishing() {
    let mut sink = subject(Status::Accepted, true);
    let id = "control_".repeat(1500);
    let records = [ControlRecord {
        id: &id,
        tier: "verify",
        reader: "unsupported",
        expect: "1",
        target: "",
        description: "private fixture",
        remedy: "",
    }];
    let refusal = validate_controls(ControlsInput::Records(&records)).unwrap_err();
    let outcome = Poll {
        markers: &Markers,
        sink: &mut sink,
        publish: |_: &posture_domain::BaselineUpdate| -> Result<(), PollStateFailure> {
            panic!("an omitted page cannot publish a healthy baseline")
        },
    }
    .run(
        TrioReading {
            values: ["1", "1", "1"],
            exit: 0,
        },
        ControlsRead::Refused(&refusal),
        None,
        LuluProfile::Base,
        None,
    );
    assert_eq!(outcome, Err(PollFailure::Gap(SubmissionFailure::Refused)));
    assert_eq!(sink.runner.requests.len(), 1);
    assert!(
        sink.runner.requests[0]
            .detail
            .starts_with("Posture security alert omitted"),
        "the omission notice stands in for the finding"
    );
}

struct Batch;
impl RunLock for Batch {
    fn taken(&self) -> bool {
        true
    }
}
impl ResultsLog for Batch {
    fn reading(&self) -> Option<LiveLog> {
        Some(LiveLog { inode: 7, size: 8 })
    }
    fn span(&self, from: u64, length: u64) -> String {
        assert_eq!((from, length), (0, 8));
        "finding\n".into()
    }
}
impl CursorStore for Batch {
    fn read(&self) -> Option<StoredCursor> {
        Some(StoredCursor {
            inode: 7,
            offset: 0,
        })
    }
    fn write(&self, _: StoredCursor) {
        panic!("an omission must not acknowledge the findings cursor")
    }
}
impl JudgeFindings for Batch {
    fn judge(&mut self, records: &str) -> JudgedBatch {
        assert_eq!(records, "finding\n");
        JudgedBatch {
            page: Some(BatchPage {
                title: "Findings".into(),
                body: "finding ".repeat(2000),
            }),
        }
    }
}

#[test]
fn an_oversized_judged_batch_reports_omission_without_advancing_its_cursor() {
    let mut sink = subject(Status::Accepted, true);
    assert_eq!(
        JudgeResults {
            lock: &Batch,
            log: &Batch,
            cursor: &Batch,
            judge: &mut Batch,
            sink: &mut sink,
            occurred_at: Some(42),
        }
        .run(),
        JudgeOutcome::Retained
    );
    assert_eq!(sink.runner.requests.len(), 1);
    assert!(
        sink.runner.requests[0]
            .detail
            .starts_with("Posture security alert omitted"),
        "the omission notice stands in for the finding"
    );
}
