mod baseline;
mod classify;
mod gaps;
pub use gaps::poll_persistence_gap;
mod render;
use crate::{Control, ControlsRefusal, Severity};
pub use baseline::{
    BaselineUpdate, ControlPrior, PollBaseline, StoredControl, Trio, TrioReading,
    trusted_poll_baseline,
};
pub use classify::{
    ControlReading, LuluProfile, classify_autologin, classify_filevault, classify_lulu_profile,
    classify_messages, classify_pgrep,
};

#[derive(Debug, Clone, Copy)]
pub struct ControlObservation<'a> {
    pub control: &'a Control,
    pub reading: ControlReading,
}
#[derive(Debug, Clone, Copy)]
pub enum ControlsRead<'a> {
    Refused(&'a ControlsRefusal),
    Observed(&'a [ControlObservation<'a>]),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollPage {
    pub severity: Severity,
    pub title: String,
    pub body: String,
    pub sound: &'static str,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollPlan {
    pub gap_members: Vec<String>,
    pub gap_page: Option<PollPage>,
    pub exposure_page: Option<PollPage>,
    pub baseline: Option<BaselineUpdate>,
}

// These are proposals, not completed writes. The owning use case must store a
// new gap page first, then any exposure page, before advancing this baseline.
// A gap on one member preserves that member without blinding clean neighbors.
pub fn plan_poll(
    trio: TrioReading<'_>,
    controls: ControlsRead<'_>,
    prior: Option<PollBaseline<'_>>,
    covered: &[&str],
    profile: LuluProfile,
) -> PollPlan {
    let clean = baseline::read_trio(trio);
    let readings: Vec<_> = match controls {
        ControlsRead::Refused(_) => vec![],
        ControlsRead::Observed(observations) => observations
            .iter()
            .map(|observation| {
                let control = observation.control;
                let value = match observation.reading {
                    ControlReading::Known(value)
                        if !control.reader.requires_target() || profile == LuluProfile::Base =>
                    {
                        control.reader.value(value.as_str())
                    }
                    _ => None,
                };
                (control, value)
            })
            .collect(),
    };
    let (gap_members, gap_page) =
        gaps::page(trio, clean.is_some(), controls, &readings, covered, profile);
    let mut plan = PollPlan {
        gap_members,
        gap_page,
        exposure_page: None,
        baseline: None,
    };
    let Some(next_trio) = clean.or(prior.map(|saved| saved.trio)) else {
        return plan;
    };
    let mut blocks = clean.map_or_else(Vec::new, |now| {
        render::trio_blocks(now, prior.map(|saved| saved.trio))
    });
    let mut next_controls = Vec::new();
    for (control, value) in readings {
        let before = baseline::prior_value(prior, control);
        if let Some(now) = value
            && now != control.expect
            && (before.is_none() || before == Some(control.expect))
        {
            blocks.push(render::control_block(control, now, before));
        }
        if let Some(next) = value.or(before) {
            next_controls.push(baseline::stored(control, next));
        }
    }
    plan.exposure_page = render::exposure_page(blocks);
    plan.baseline = Some(BaselineUpdate {
        trio: next_trio,
        controls: next_controls,
        preserve_prior_fields: matches!(controls, ControlsRead::Refused(_)) && prior.is_some(),
    });
    plan
}

#[cfg(test)]
mod tests;
