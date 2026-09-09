use crate::lanes::stubs::stub_facts;
use crate::runner::SystemRunner;
use crate::{CommandRunner, LaneAdapter, RotateLogsLane};
use std::fs::{self, DirBuilder, File};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

pub(super) fn sandbox() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "uu-rotation-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    root
}
pub(super) fn runner() -> SystemRunner {
    SystemRunner::for_lane(
        "rotation",
        Duration::from_millis(400),
        Duration::from_millis(400),
    )
}
pub(super) fn lane(logs: &[&Path]) -> RotateLogsLane {
    RotateLogsLane {
        logs: logs
            .iter()
            .map(|p| p.to_str().unwrap().to_owned())
            .collect(),
        rotate_at_bytes: 16,
        archives_kept: 3,
        compressor: "/usr/bin/gzip".into(),
    }
}
pub(super) fn run(lane: &RotateLogsLane) -> uu_domain::LaneReport {
    lane.run("named-rotation", &stub_facts(), &runner())
}
pub(super) fn archive(log: &Path, index: u64) -> PathBuf {
    PathBuf::from(format!("{}.{index}.gz", log.display()))
}
pub(super) fn bytes(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}
pub(super) fn inflate(path: &Path) -> Vec<u8> {
    let output = path.with_extension("decoded");
    runner()
        .run_to_file(
            "/usr/bin/gzip",
            &["-dc"],
            File::open(path).unwrap(),
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&output)
                .unwrap(),
        )
        .unwrap();
    bytes(&output)
}
