//! Starting, stopping and inspecting the daemon's own launchd service.

/// One controller over a launchd service, named by its label. The command
/// layer owns which verb runs and what it prints; this is only the seam that
/// lets it be tested against a scripted double instead of real launchd.
pub trait ServiceController {
    fn start(&self, label: &str) -> Result<(), ServiceError>;
    fn stop(&self, label: &str) -> Result<(), ServiceError>;
    fn restart(&self, label: &str) -> Result<(), ServiceError>;
    fn status(&self, label: &str) -> Result<ServiceState, ServiceError>;
}

/// What `status` found. `NotLoaded` is a reading here, not a failure: `print`
/// answers it the same way it answers every other state, on its own exit 0
/// stdout line, and only the command layer turns it into exit 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceState {
    Running { pid: u32 },
    Loaded,
    NotLoaded,
}

/// Why a launchctl call did not do what it was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceError {
    /// The plist `start` would bootstrap from is not on disk, so it refuses
    /// before running launchctl at all. Carries the path it looked for.
    MissingPlist(String),
    /// `restart` and `stop` each read their own not-loaded case off launchctl's
    /// own refusal: `restart` falls back to `start`, `stop` reads it as
    /// already done. `status` never returns this; it answers with
    /// `ServiceState::NotLoaded` instead, because for `status` that is the
    /// question's own answer rather than a failure to run it.
    NotLoaded,
    /// Every other launchctl failure, stderr trimmed.
    Failed(String),
}
