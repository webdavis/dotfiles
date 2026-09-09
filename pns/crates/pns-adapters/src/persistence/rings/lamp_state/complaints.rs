use super::*;
use pns_application::{LampComplaint, LampComplaints};

fn marker(kind: LampComplaint) -> &'static str {
    match kind {
        LampComplaint::Tick => LIGHTS_SAID,
        LampComplaint::Quiet => LIGHTS_QUIET_SAID,
    }
}
impl LampComplaints for FileLampState {
    fn remembered(&self, kind: LampComplaint) -> String {
        std::fs::read_to_string(self.state.join(marker(kind))).unwrap_or_default()
    }
    fn remember(&self, kind: LampComplaint, said: Option<&str>) {
        let path = self.state.join(marker(kind));
        match said {
            Some(said) => {
                let _ = publish_state_line(&path, said);
            }
            None => {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}
/// Where a tick remembers what it last complained about.
pub const LIGHTS_SAID: &str = "lights-said";
