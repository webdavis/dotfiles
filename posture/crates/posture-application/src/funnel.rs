use crate::{Alert, AlertSignal, AlertSink, Submission, SubmissionFailure};
use posture_domain::{
    FUNNEL_CRITICAL_TITLE, FunnelAlert, FunnelBaseline, FunnelReadFailure, FunnelReading,
    FunnelState, funnel_corruption_gap, funnel_persistence_gap, funnel_read_gap, plan_funnel,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelGap {
    Readings,
    Persistence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunnelStateFailure;
pub trait FunnelStore {
    fn covered(&self, gap: FunnelGap) -> bool;
    fn remember(&self, gap: FunnelGap) -> Result<(), FunnelStateFailure>;
    fn clear(&self, gap: FunnelGap) -> Result<(), FunnelStateFailure>;
    fn publish(&self, state: FunnelState) -> Result<(), FunnelStateFailure>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelFailure {
    ReadGap(SubmissionFailure),
    Exposure(SubmissionFailure),
    /// The corrupt-baseline warning was raised and no destination took it.
    /// Reported after the repair rather than instead of it.
    CorruptBaseline(SubmissionFailure),
    Persistence,
}
pub struct Funnel<'a, M, S> {
    pub store: &'a M,
    pub sink: &'a mut S,
}
impl<M: FunnelStore, S: AlertSink> Funnel<'_, M, S> {
    pub fn run(
        &mut self,
        reading: Result<FunnelReading, FunnelReadFailure>,
        baseline: FunnelBaseline,
        occurred_at: Option<u64>,
    ) -> Result<(), FunnelFailure> {
        let mut corruption_lost = None;
        let value = reading.as_ref().unwrap_or(&FunnelReading::Gap);
        let plan = plan_funnel(value, baseline, self.store.covered(FunnelGap::Readings));
        if plan.clear_read_gap {
            let _ = self.store.clear(FunnelGap::Readings);
        }
        match plan.alert {
            Some(FunnelAlert::ReadGap) => {
                let failure = reading.err().unwrap_or(FunnelReadFailure::UnexpectedShape);
                self.gap(FunnelGap::Readings, funnel_read_gap(&failure), occurred_at)
                    .map_err(FunnelFailure::ReadGap)?;
            }
            Some(FunnelAlert::Exposure(body)) => self
                .submit("page", body, occurred_at)
                .map_err(FunnelFailure::Exposure)?,
            Some(FunnelAlert::CorruptBaseline) => {
                // The repair follows even a refused warning, so the refusal is
                // carried to the end of the run rather than short-circuiting it.
                corruption_lost = self
                    .submit("gap", funnel_corruption_gap(), occurred_at)
                    .err();
            }
            None => {}
        }
        let Some(next) = plan.next else {
            return lost(corruption_lost);
        };
        if self.store.publish(next).is_err() {
            let _ = self.gap(
                FunnelGap::Persistence,
                funnel_persistence_gap(),
                occurred_at,
            );
            return Err(FunnelFailure::Persistence);
        }
        let _ = self.store.clear(FunnelGap::Persistence);
        lost(corruption_lost)
    }
    fn gap(
        &mut self,
        kind: FunnelGap,
        body: String,
        occurred_at: Option<u64>,
    ) -> Result<(), SubmissionFailure> {
        if !self.store.covered(kind) {
            self.submit("gap", body, occurred_at)?;
            // A failed marker write can repeat a page, but cannot acknowledge an uncommitted page.
            let _ = self.store.remember(kind);
        }
        Ok(())
    }
    fn submit(
        &mut self,
        event: &'static str,
        detail: String,
        occurred_at: Option<u64>,
    ) -> Result<(), SubmissionFailure> {
        match self.sink.submit(&Alert {
            occurrence_id: None,
            event,
            signal: AlertSignal::NeedsAttention,
            severity: None,
            occurred_at,
            title: FUNNEL_CRITICAL_TITLE.into(),
            detail,
        }) {
            Submission::Accepted => Ok(()),
            Submission::NotAccepted(error) => Err(error),
        }
    }
}
/// A run that finished its work still fails when its corruption warning did.
fn lost(corruption: Option<SubmissionFailure>) -> Result<(), FunnelFailure> {
    match corruption {
        Some(failure) => Err(FunnelFailure::CorruptBaseline(failure)),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
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
}
