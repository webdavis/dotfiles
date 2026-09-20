use super::*;
use std::cell::RefCell;

#[derive(Default)]
struct Store {
    published: RefCell<Vec<FunnelState>>,
}
impl FunnelStore for Store {
    fn covered(&self, _gap: FunnelGap) -> bool {
        false
    }
    fn remember(&self, _gap: FunnelGap) -> Result<(), FunnelStateFailure> {
        Ok(())
    }
    fn clear(&self, _gap: FunnelGap) -> Result<(), FunnelStateFailure> {
        Ok(())
    }
    fn publish(&self, state: FunnelState) -> Result<(), FunnelStateFailure> {
        self.published.borrow_mut().push(state);
        Ok(())
    }
}
struct Sink(Submission);
impl AlertSink for Sink {
    fn submit(&mut self, _alert: &Alert) -> Submission {
        self.0
    }
}

fn repair(result: Submission) -> (Result<(), FunnelFailure>, Vec<FunnelState>) {
    let store = Store::default();
    let mut sink = Sink(result);
    let outcome = Funnel {
        store: &store,
        sink: &mut sink,
    }
    .run(
        Ok(FunnelReading::Inactive),
        FunnelBaseline::Corrupt,
        Some(7),
    );
    let published = store.published.borrow().clone();
    (outcome, published)
}

#[test]
fn a_repaired_baseline_whose_corruption_warning_was_refused_is_reported_as_a_failure() {
    let (outcome, published) = repair(Submission::NotAccepted(SubmissionFailure::Refused));
    assert_eq!(
        outcome,
        Err(FunnelFailure::CorruptBaseline(SubmissionFailure::Refused))
    );
    // The repair is what the warning must never block, so it still happened.
    assert_eq!(published, [FunnelState::Inactive]);
}

#[test]
fn a_delivered_corruption_warning_leaves_the_run_successful() {
    let (outcome, published) = repair(Submission::Accepted);
    assert_eq!(outcome, Ok(()));
    assert_eq!(published, [FunnelState::Inactive]);
}
