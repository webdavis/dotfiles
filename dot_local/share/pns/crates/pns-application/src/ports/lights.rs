use pns_domain::lights::mute::Muted;

/// One typed read serves the command's repair and the lamp path's dark refusal.
pub trait LampMutes {
    fn read(&self) -> (Vec<Muted>, Vec<String>);
    fn write(&self, entries: &[Muted]) -> Result<(), String>;
}

pub trait LoopLeases {
    fn begin(&self, pane: &str, now: u64) -> Result<(), String>;
    fn end(&self, pane: &str) -> Result<(), String>;
}

#[derive(Clone, Copy)]
pub enum LampComplaint {
    Tick,
    Quiet,
}
pub trait LampComplaints {
    fn remembered(&self, kind: LampComplaint) -> String;
    fn remember(&self, kind: LampComplaint, said: Option<&str>);
}
