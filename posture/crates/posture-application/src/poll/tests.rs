use super::*;
use crate::{Alert, AlertSignal, Submission};
use posture_domain::{ControlsInput, trusted_poll_baseline, validate_controls};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Default)]
struct Recorded {
    calls: Vec<String>,
    readings: Vec<String>,
    persistence: Vec<String>,
    baseline: [u8; 3],
    alerts: Vec<Alert>,
}
#[derive(Clone)]
struct Fixture {
    recorded: Rc<RefCell<Recorded>>,
    submissions: VecDeque<Submission>,
    marker_refusal: bool,
}
impl PollMarkers for Fixture {
    fn covered(&self, gap: PollGap) -> Vec<String> {
        let mut r = self.recorded.borrow_mut();
        r.calls.push(format!("covered:{gap:?}"));
        match gap {
            PollGap::Readings => r.readings.clone(),
            PollGap::Persistence => r.persistence.clone(),
        }
    }
    fn remember(&self, gap: PollGap, members: &[String]) -> Result<(), PollStateFailure> {
        let mut r = self.recorded.borrow_mut();
        r.calls
            .push(format!("remember:{gap:?}:{}", members.join(" ")));
        if self.marker_refusal {
            return Err(PollStateFailure);
        }
        match gap {
            PollGap::Readings => r.readings = members.to_vec(),
            PollGap::Persistence => r.persistence = members.to_vec(),
        }
        Ok(())
    }
    fn clear(&self, gap: PollGap) -> Result<(), PollStateFailure> {
        let mut r = self.recorded.borrow_mut();
        r.calls.push(format!("clear:{gap:?}"));
        if self.marker_refusal {
            return Err(PollStateFailure);
        }
        match gap {
            PollGap::Readings => r.readings.clear(),
            PollGap::Persistence => r.persistence.clear(),
        }
        Ok(())
    }
}
impl AlertSink for Fixture {
    fn submit(&mut self, alert: &Alert) -> Submission {
        let mut r = self.recorded.borrow_mut();
        r.calls.push(format!("submit:{}", alert.event));
        r.alerts.push(alert.clone());
        self.submissions.pop_front().unwrap_or(Submission::Accepted)
    }
}
fn fixture() -> Fixture {
    Fixture {
        recorded: Rc::new(RefCell::new(Recorded {
            baseline: [1, 1, 1],
            ..Default::default()
        })),
        submissions: VecDeque::new(),
        marker_refusal: false,
    }
}
#[derive(Clone, Copy)]
enum Publication {
    Success,
    BeforeRename,
    AfterRename,
}
fn execute(
    fixture: &mut Fixture,
    trio: [&str; 3],
    refused: bool,
    prior: bool,
    publication: Publication,
) -> Result<(), PollFailure> {
    let markers = fixture.clone();
    let output = fixture.recorded.clone();
    let mut poll = Poll {
        markers: &markers,
        sink: fixture,
        publish: move |update: &BaselineUpdate| {
            let mut r = output.borrow_mut();
            r.calls.push("publish".into());
            if matches!(publication, Publication::BeforeRename) {
                return Err(PollStateFailure);
            }
            r.baseline = update.trio.values();
            if matches!(publication, Publication::AfterRename) {
                return Err(PollStateFailure);
            }
            Ok(())
        },
    };
    let refusal = validate_controls(ControlsInput::Missing("/fixture/controls")).unwrap_err();
    let controls = if refused {
        ControlsRead::Refused(&refusal)
    } else {
        ControlsRead::Observed(&[])
    };
    let prior = if prior {
        trusted_poll_baseline(Some(0o600), true, ["1", "1", "1"], &[])
    } else {
        None
    };
    poll.run(
        TrioReading {
            values: trio,
            exit: 0,
        },
        controls,
        prior,
        LuluProfile::Base,
        Some(42),
    )
}
const FAILURES: [SubmissionFailure; 6] = [
    SubmissionFailure::Unavailable,
    SubmissionFailure::Failed,
    SubmissionFailure::TimedOut,
    SubmissionFailure::Unparseable,
    SubmissionFailure::Refused,
    SubmissionFailure::NotCommitted,
];
mod gap;
mod publication;
