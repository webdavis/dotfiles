use pns_application::LampMarkers;
use std::path::PathBuf;

pub struct FileLampMarkers(pub PathBuf);
impl LampMarkers for FileLampMarkers {
    fn sweep_legacy(&self) {
        super::sweep_legacy_state(&self.0);
    }
    fn shell_since(&self) -> Option<u64> {
        super::sweep_shell_markers(&self.0)
    }
    fn leases(&self, now: u64, timeout_secs: u64) -> Vec<u64> {
        super::sweep_leases(&self.0, now, timeout_secs)
    }
    fn blocked(&self, now: u64, give_up_after_secs: u64) -> Vec<u64> {
        super::sweep::sweep_blocked(&self.0, now, give_up_after_secs)
    }
}
