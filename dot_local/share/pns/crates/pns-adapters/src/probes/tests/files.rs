use super::*;

pub(super) fn probes_answering(answer: &str) -> SystemProbes<FakeRunner> {
    SystemProbes::new(FakeRunner::answering(answer), "/marker".to_string())
}

pub(super) fn probes_failing() -> SystemProbes<FakeRunner> {
    SystemProbes::new(FakeRunner::failing(), "/marker".to_string())
}

// --- the presence reading -----------------------------------------------

/// A scratch path of this test's own, removed first so a previous run
/// cannot seed it.
pub(super) fn scratch_path(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("pns-{name}-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    path
}

pub(super) fn probes_at(path: &std::path::Path) -> SystemProbes<FakeRunner> {
    SystemProbes::new(FakeRunner::failing(), "/marker".to_string())
        .with_presence_path(path.to_string_lossy().into_owned())
}

// --- the phone's input clock -------------------------------------------

/// The probe with the three discovery answers scripted by exact argv,
/// pointed at a marker path nothing reads.
pub(super) fn phone_probe(answers: &[(&str, &str)]) -> SystemProbes<ExactArgvRunner> {
    SystemProbes::new(
        ExactArgvRunner {
            answers: answers
                .iter()
                .map(|(call, out)| ((*call).to_string(), (*out).to_string()))
                .collect(),
            calls: Mutex::new(Vec::new()),
        },
        "/marker".to_string(),
    )
}

// --- newest_terminal_atime, against files whose atimes are set ----------

/// A file whose access time is exactly this many seconds past the epoch.
///
/// AN ABSOLUTE INSTANT, never a wall-clock stamp. This used to shell out
/// to `touch -a -t`, which reads its stamp in the HOST'S LOCAL TIME, so
/// the fixture meant one epoch on the developer's machine and another on
/// a UTC runner: the same two assertions passed in Denver and failed in
/// CI, seven hours apart. The probe under test reports epoch seconds, so
/// the fixture states epoch seconds and the assertion reads the same
/// constant back.
pub(super) fn terminal_with_atime(dir: &std::path::Path, name: &str, atime_secs: u64) {
    let file = std::fs::File::create(dir.join(name)).expect("terminal fixture");
    file.set_times(
        std::fs::FileTimes::new()
            .set_accessed(std::time::UNIX_EPOCH + std::time::Duration::from_secs(atime_secs)),
    )
    .expect("set the fixture atime");
}
