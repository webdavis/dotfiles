use super::*;

// --- the reminder ----------------------------------------------------------------
//
// The feature's own harness. A record is written BY HAND here rather than
// through `pns_adapters::remind_records::render`, so the on-disk form is pinned by something other
// than the writer under test, and the channel stubs COUNT their invocations,
// because "exactly one card" is the property most of these behaviors turn on.

/// The three stub channels enabled, plus the reminder scheduled (or, at zero,
/// off) and switched on for the producer these fixtures send as.
///
/// THE PRODUCER'S ENTRY IS WHAT ARMS IT HERE. Nothing arms a reminder for a
/// name alone any more, so a fixture that wants one either writes this table
/// or passes `--remind` on the call.
pub(crate) fn remind_config(delay_secs: u64) -> String {
    format!(
        "{}[producer.claude]\nremind = true\n[remind]\ndelay = \"{delay_secs}s\"\n",
        support::STUB_CHANNELS
    )
}

/// Channels that record the last event AND count how many arrived.
///
/// THE COUNT IS THE POINT. `Sandbox::new`'s stub truncates, so two deliveries
/// leave one file and "exactly one card" is unfalsifiable through it. One line
/// appended per invocation answers the question the coalescing ruling asks.
///
/// THE LINE CARRIES `PNS_REQUEST_ID`, which is the engine's own name for the
/// EVENT behind a delivery, so a caller can ask how many events were carded
/// rather than only how many attempts were made. The two differ whenever a
/// live daemon is ticking beside the fire: a channel script can never confirm
/// a delivery (`deliver_executable` answers `Silent` whatever the script
/// exits), so its leg stays retry-eligible from the moment it is written and
/// `pns gateway retry` re-delivers the same event on a later tick.
pub(crate) fn counted_channels(sandbox: &Sandbox) {
    for channel in ["phone", "hermes", "banner"] {
        sandbox.stub_channel(
            channel,
            &format!(
                "printf '%s\\n' \"${{PNS_REQUEST_ID:?}}\" >>\"{s}/{channel}.count\"; \
                 cat >\"{s}/{channel}.event\"",
                s = sandbox.display()
            ),
        );
    }
}

/// How many times one counted channel was handed an event, retries included.
pub(crate) fn deliveries(sandbox: &Sandbox, channel: &str) -> usize {
    delivered_requests(sandbox, channel).len()
}

/// How many DISTINCT events one counted channel was handed, which is the
/// question "exactly one card" is really asking wherever a daemon's delivery
/// retry can hand the same event over a second time.
pub(crate) fn carded_events(sandbox: &Sandbox, channel: &str) -> usize {
    delivered_requests(sandbox, channel)
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn delivered_requests(sandbox: &Sandbox, channel: &str) -> Vec<String> {
    std::fs::read_to_string(sandbox.path(&format!("{channel}.count")))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

pub(crate) fn remind_record(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/remind/{session}.pending"))
}

pub(crate) fn remind_marker(sandbox: &Sandbox, session: &str) -> std::path::PathBuf {
    sandbox.path(&format!("state/daemon-markers/remind-{session}"))
}

/// Every name the reminder directory holds, which is how a test sees the working
/// files a fire is supposed to clean up after itself.
pub(crate) fn remind_directory_names(sandbox: &Sandbox) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(sandbox.path("state/remind"))
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
    let path = remind_record(sandbox, session);
    std::fs::create_dir_all(path.parent().expect("the reminder directory"))
        .expect("the reminder directory");
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
    let path = remind_marker(sandbox, session);
    std::fs::create_dir_all(path.parent().expect("the marker directory")).expect("markers");
    std::fs::write(&path, "").expect("the marker");
}

/// `pns remind`, against this sandbox's own state directory and stubs.
pub(crate) fn remind(sandbox: &Sandbox) -> Command {
    let mut command = sandbox.pns_stateful();
    command.arg("remind");
    command
}

/// The daemon's spool entry for one session's nudge job, as the daemon's own
/// on-disk form. COUPLED TO THAT FORM DELIBERATELY and named as such: if the
/// daemon ever exposes a read helper, this is the one place to re-point.
pub(crate) fn spool_entry(sandbox: &Sandbox, session: &str) -> String {
    std::fs::read_to_string(sandbox.path(&format!("state/daemon/remind:{session}")))
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
    let table = match file {
        "activity" => "activity",
        "decisions" => "decisions",
        "missed-notifications" => "journal",
        _ => panic!("unrecognized stored record fixture: {file}"),
    };
    stored_records::lines(sandbox, table)
}
