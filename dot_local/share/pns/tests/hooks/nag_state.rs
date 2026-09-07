use super::*;

// --- the nag ----------------------------------------------------------------
//
// The feature's own harness. A record is written BY HAND here rather than
// through `pns::nag::render`, so the on-disk form is pinned by something other
// than the writer under test, and the channel stubs COUNT their invocations,
// because "exactly one card" is the property most of these behaviors turn on.

/// The three stub channels enabled, plus the nag scheduled (or, at zero, off).
pub(crate) fn nag_config(after_secs: u64) -> String {
    format!(
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n[plugins.hermes]\nenabled = true\n\
         [plugins.macos-banner]\nenabled = true\n[nag]\nafter_secs = {after_secs}\n"
    )
}

/// Channels that record the last event AND count how many arrived.
///
/// THE COUNT IS THE POINT. `Sandbox::new`'s stub truncates, so two deliveries
/// leave one file and "exactly one card" is unfalsifiable through it. One byte
/// appended per invocation answers the question the coalescing ruling asks.
pub(crate) fn counted_channels(sandbox: &Sandbox) {
    for channel in ["mobile", "hermes", "macos-banner"] {
        sandbox.stub_channel(
            channel,
            &format!(
                "printf 'x' >>\"{s}/{channel}.count\"; cat >\"{s}/{channel}.event\"",
                s = sandbox.display()
            ),
        );
    }
}

/// How many events one counted channel was handed.
pub(crate) fn deliveries(sandbox: &Sandbox, channel: &str) -> usize {
    std::fs::read_to_string(sandbox.path(&format!("{channel}.count")))
        .unwrap_or_default()
        .len()
}

pub(crate) fn nag_record(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/nag/{session}.pending"))
}

pub(crate) fn nag_marker(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/daemon-markers/nag-{session}"))
}

/// Every name the nag directory holds, which is how a test sees the working
/// files a fire is supposed to clean up after itself.
pub(crate) fn nag_directory_names(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state/nag"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

pub(crate) fn epoch_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
}

/// One outstanding approval on disk, armed `waited` seconds ago.
pub(crate) fn write_record(
    sandbox: &Sandbox,
    session: &str,
    waited: u64,
    detail: &str,
    pane: &str,
) {
    write_record_at(sandbox, session, epoch_now() - waited, detail, pane);
}

/// The same, at an epoch the caller states, which is how a record armed in the
/// FUTURE is written.
pub(crate) fn write_record_at(
    sandbox: &Sandbox,
    session: &str,
    armed: u64,
    detail: &str,
    pane: &str,
) {
    let path = nag_record(sandbox, session);
    std::fs::create_dir_all(path.parent().expect("the nag directory")).expect("the nag directory");
    std::fs::write(
        &path,
        serde_json::json!({
            "agent": "claude",
            "project": "dotfiles",
            "branch": "",
            "detail": detail,
            "pane": pane,
            "armed": armed,
        })
        .to_string(),
    )
    .expect("the record");
}

pub(crate) fn write_marker(sandbox: &Sandbox, session: &str) {
    let path = nag_marker(sandbox, session);
    std::fs::create_dir_all(path.parent().expect("the marker directory")).expect("markers");
    std::fs::write(&path, "").expect("the marker");
}

/// `pns nag`, against this sandbox's own state directory and stubs.
pub(crate) fn nag(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.pns_stateful();
    command.arg("nag");
    command
}

/// The daemon's spool entry for one session's nudge job, as the daemon's own
/// on-disk form. COUPLED TO THAT FORM DELIBERATELY and named as such: if the
/// daemon ever exposes a read helper, this is the one place to re-point.
pub(crate) fn spool_entry(sandbox: &Sandbox, session: &str) -> String {
    std::fs::read_to_string(sandbox.path(&format!("state/daemon/nag:{session}")))
        .unwrap_or_default()
}

pub(crate) fn spool_entries(sandbox: &Sandbox) -> Vec<String> {
    std::fs::read_dir(sandbox.path("state/daemon"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

pub(crate) fn state_lines(sandbox: &Sandbox, file: &str) -> Vec<String> {
    std::fs::read_to_string(sandbox.path(&format!("state/{file}")))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}
