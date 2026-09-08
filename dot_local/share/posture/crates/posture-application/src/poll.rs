use crate::{Alert, AlertSignal, AlertSink, PollGap, Submission, SubmissionFailure};
use posture_domain::{
    BaselineUpdate, ControlsRead, LuluProfile, PollBaseline, PollPage, TrioReading, plan_poll,
    poll_persistence_gap,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PollStateFailure;
pub trait PollMarkers {
    fn covered(&self, gap: PollGap) -> Vec<String>;
    fn remember(&self, gap: PollGap, members: &[String]) -> Result<(), PollStateFailure>;
    fn clear(&self, gap: PollGap) -> Result<(), PollStateFailure>;
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollFailure {
    Gap(SubmissionFailure),
    Exposure(SubmissionFailure),
    Persistence,
}
pub struct Poll<'a, M, S, P> {
    pub markers: &'a M,
    pub sink: &'a mut S,
    pub publish: P,
}
impl<M: PollMarkers, S: AlertSink, P: FnMut(&BaselineUpdate) -> Result<(), PollStateFailure>>
    Poll<'_, M, S, P>
{
    pub fn run(
        &mut self,
        trio: TrioReading<'_>,
        controls: ControlsRead<'_>,
        prior: Option<PollBaseline<'_>>,
        profile: LuluProfile,
        occurred_at: Option<u64>,
    ) -> Result<(), PollFailure> {
        let covered = self.markers.covered(PollGap::Readings);
        let covered: Vec<_> = covered.iter().map(String::as_str).collect();
        let plan = plan_poll(trio, controls, prior, &covered, profile);
        if plan.gap_members.is_empty() {
            let _ = self.markers.clear(PollGap::Readings);
        } else {
            self.gap(
                PollGap::Readings,
                &plan.gap_members,
                plan.gap_page.as_ref(),
                occurred_at,
            )
            .map_err(PollFailure::Gap)?;
        }
        let Some(baseline) = plan.baseline else {
            return Ok(());
        };
        if let Some(page) = plan.exposure_page {
            self.submit("page", &page, occurred_at)
                .map_err(PollFailure::Exposure)?;
        }
        if (self.publish)(&baseline).is_err() {
            let covered = self.markers.covered(PollGap::Persistence);
            let page = (!covered.iter().any(|member| member == "baseline_persist"))
                .then(poll_persistence_gap);
            // This separate marker never makes a failed publication successful. Its
            // own submission can fail too; the next poll must remain eligible to retry.
            let _ = self.gap(
                PollGap::Persistence,
                &["baseline_persist".into()],
                page.as_ref(),
                occurred_at,
            );
            return Err(PollFailure::Persistence);
        }
        let _ = self.markers.clear(PollGap::Persistence);
        Ok(())
    }
    fn gap(
        &mut self,
        kind: PollGap,
        members: &[String],
        page: Option<&PollPage>,
        occurred_at: Option<u64>,
    ) -> Result<(), SubmissionFailure> {
        if let Some(page) = page {
            self.submit("gap", page, occurred_at)?;
        }
        // A covered gap refreshes to its recovered member set without another page.
        // A refused marker write can cause a repeat page, never a false acknowledgement.
        let _ = self.markers.remember(kind, members);
        Ok(())
    }
    fn submit(
        &mut self,
        event: &'static str,
        page: &PollPage,
        occurred_at: Option<u64>,
    ) -> Result<(), SubmissionFailure> {
        match self.sink.submit(&Alert {
            occurrence_id: None,
            event,
            signal: AlertSignal::NeedsAttention,
            occurred_at,
            title: page.title.clone(),
            detail: page.body.clone(),
        }) {
            Submission::Accepted => Ok(()),
            Submission::NotAccepted(failure) => Err(failure),
        }
    }
}
#[cfg(test)]
mod tests;
