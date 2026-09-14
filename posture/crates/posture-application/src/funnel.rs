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
                // The Bash corruption warning is best effort; repair follows even a refused warning.
                let _ = self.submit("gap", funnel_corruption_gap(), occurred_at);
            }
            None => {}
        }
        let Some(next) = plan.next else {
            return Ok(());
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
        Ok(())
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
