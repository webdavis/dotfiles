use posture_adapters::Delivery;
use posture_domain::HeartbeatWindow;
use std::{ffi::OsString, path::PathBuf};
pub(super) struct Configuration {
    pub snapshots: PathBuf,
    pub delivery: Delivery,
    pub alarm: PathBuf,
    pub maximum_age: HeartbeatWindow,
}
impl Configuration {
    pub(super) fn read(mut variable: impl FnMut(&str) -> Option<OsString>) -> Option<Self> {
        let home = variable("HOME")?;
        let mut default_snapshots = home.clone();
        default_snapshots.push("/.local/log/osquery/osqueryd.snapshots.log");
        let delivery = Delivery::read(std::path::Path::new(&home));
        let bound = variable("OSQUERY_CANARY_MAX_AGE");
        Some(Self {
            snapshots: default_snapshots.into(),
            delivery,
            alarm: "/usr/bin/osascript".into(),
            maximum_age: HeartbeatWindow::from_override(
                bound.as_deref().and_then(|value| value.to_str()),
            ),
        })
    }
}
#[cfg(test)]
mod tests;
