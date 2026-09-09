use posture_application::{PollGap, SavedPollState};
use posture_domain::Control;
use std::{fs, path::PathBuf};

pub struct PollStateFiles {
    baseline: PathBuf,
    prior_json: Option<String>,
}
impl PollStateFiles {
    pub fn new(baseline: PathBuf) -> Self {
        Self {
            baseline,
            prior_json: None,
        }
    }
    fn sibling(&self, suffix: &str) -> PathBuf {
        let mut path = self.baseline.as_os_str().to_owned();
        path.push(suffix);
        path.into()
    }
    fn marker(&self, gap: PollGap) -> PathBuf {
        self.sibling(match gap {
            PollGap::Readings => ".gap",
            PollGap::Persistence => ".persist-gap",
        })
    }
    pub fn read(&mut self, controls: &[Control]) -> Option<SavedPollState> {
        self.prior_json = None;
        let (reading, json) = baseline::read(&self.baseline, controls)?;
        self.prior_json = Some(json);
        Some(reading)
    }
}
impl Drop for PollStateFiles {
    fn drop(&mut self) {
        // The original invocation's EXIT trap removes its fixed sibling temporary.
        // A directory or unavailable parent still refuses removal, best effort.
        let _ = fs::remove_file(self.sibling(".tmp"));
    }
}
mod baseline;
mod encoding;
mod markers;
mod publication;
#[cfg(test)]
mod tests;
