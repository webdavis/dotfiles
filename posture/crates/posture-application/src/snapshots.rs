use posture_domain::CanaryEpoch;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotReadFailure;
pub trait SnapshotsLog {
    fn newest_canary(&mut self) -> Result<Option<CanaryEpoch>, SnapshotReadFailure>;
}
