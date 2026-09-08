use pns_domain::lights::{streak::Streak, unread::News};

pub trait AgentWork {
    fn statuses(&self) -> Vec<String>;
}
pub trait LampMarkers {
    fn sweep_legacy(&self);
    fn shell_since(&self) -> Option<u64>;
    fn leases(&self, now: u64, timeout_secs: u64) -> Vec<u64>;
    fn blocked(&self, now: u64, give_up_after_secs: u64) -> Vec<u64>;
}
pub trait LampHouseRecords {
    fn advance_streak(&self, working: bool, now: u64) -> Option<Streak>;
    fn news(&self) -> News;
}
