use posture_domain::{ControlPrior, PollBaseline, trusted_poll_baseline};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPollControl {
    pub id: String,
    pub value: String,
    pub expect: String,
    pub target: String,
}
impl SavedPollControl {
    pub fn prior(&self) -> ControlPrior<'_> {
        ControlPrior {
            id: &self.id,
            value: &self.value,
            expect: &self.expect,
            target: &self.target,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedPollState {
    pub values: [String; 3],
    pub controls: Vec<SavedPollControl>,
}
impl SavedPollState {
    pub fn baseline<'a>(&self, controls: &'a [ControlPrior<'a>]) -> Option<PollBaseline<'a>> {
        trusted_poll_baseline(
            Some(0o600),
            true,
            self.values.each_ref().map(String::as_str),
            controls,
        )
    }
}
#[derive(Debug, Clone, Copy)]
pub enum PollGap {
    Readings,
    Persistence,
}
