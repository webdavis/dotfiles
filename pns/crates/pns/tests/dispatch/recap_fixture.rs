use super::*;

// --- the summarizer ---------------------------------------------------------

/// The command word a test's summarizer stub answers to. A BARE NAME RESOLVED
/// THROUGH PATH, which is the shape the real key takes (`["ollama", "run",
/// ...]`), so a test exercises the same resolution the operator's own config
/// will.
pub(super) const SUMMARIZER: &str = "recap-summarizer";

/// A config naming that stub as the summarizer, plus whatever else the test
/// needs inside `[recap]`.
pub(super) fn recap_summarized_by(extra: &str) -> String {
    format!("{EVERY_DISPATCHED_CHANNEL}[recap]\nsummarizer = [\"{SUMMARIZER}\"]\n{extra}")
}

/// A summarizer stub first on PATH. EVERY BODY DRAINS STDIN FIRST, because the
/// engine writes the prompt into the pipe and a stub that never read it would
/// be measuring the writer rather than itself.
pub(super) fn stub_summarizer(sandbox: &Sandbox, command: &mut std::process::Command, body: &str) {
    sandbox.stub_on_path(command, SUMMARIZER, &format!("cat >/dev/null\n{body}"));
}

/// The recap the detached child posted, waited for rather than slept on.
pub(super) fn posted_recap(sandbox: &Sandbox) -> String {
    poll_until(|| {
        events(sandbox, "hermes")
            .into_iter()
            .find(|event| event["state"] == "recap")
    })
    .unwrap_or_else(|| {
        panic!(
            "no recap reached the durable route: {:?}",
            events(sandbox, "hermes")
        )
    })["detail"]
        .as_str()
        .expect("a detail")
        .to_string()
}

/// The recap CARD the detached child dispatched, waited for rather than slept
/// on, and the live events raised beside it.
///
/// THE CARD IS THE CHILD'S NOW. It is composed at the return moment and handed
/// to the recap child, which dispatches it at once, so it arrives from the
/// child after the event that triggered it has exited rather than inside it.
pub(super) fn carded_recap(sandbox: &Sandbox) -> (serde_json::Value, Vec<serde_json::Value>) {
    let card = poll_until(|| {
        events(sandbox, "banner")
            .into_iter()
            .find(|event| event["state"] == "missed")
    })
    .unwrap_or_else(|| {
        panic!(
            "no recap card was dispatched: {:?}",
            events(sandbox, "banner")
        )
    });
    (card, events(sandbox, "banner"))
}

/// What a recap says when the summarizer it was told to use produced nothing.
/// One sentence for every way of failing, so a test names the outcome rather
/// than the mechanism.
pub(super) const DID_NOT_ANSWER: &str = "did not answer";

/// The mechanical timeline is back, and the message says which of the two plain
/// lists this is. Shared by the three ways of saying nothing.
pub(super) fn assert_fell_back_to_the_plain_list(body: &str) {
    assert!(
        body.contains("- planted 0  claude  b  0m  done"),
        "the mechanical list did not come back: {body}"
    );
    assert!(
        body.contains(DID_NOT_ANSWER),
        "the plain list did not say it was the fallback: {body}"
    );
}

/// A recap over a loud window with a summarizer that behaves as `body` says,
/// posted and read back.
pub(super) fn recap_summarized_badly(name: &str, extra: &str, body: &str) -> String {
    let sandbox = Sandbox::new(name);
    record_every_event(&sandbox);
    sandbox.write_config(&recap_summarized_by(extra));
    loud_window(&sandbox);
    let mut command = present_event(&sandbox);
    stub_summarizer(&sandbox, &mut command, body);
    run(&mut command);
    posted_recap(&sandbox)
}

// --- the two sections whose source is not pns -------------------------------

/// The command word a test's `pull_requests` source answers to. A BARE NAME
/// RESOLVED THROUGH PATH, which is the shape the real key takes, so a test
/// exercises the same resolution the operator's own config will.
pub(super) const PULL_REQUESTS: &str = "recap-pull-requests";

/// A config naming that stub as the `pull_requests` source, plus whatever else
/// the test needs inside `[recap]`.
pub(super) fn recap_sourced_from(extra: &str) -> String {
    format!(
        "{EVERY_DISPATCHED_CHANNEL}[recap]\n{extra}\n[recap.sources]\n\
         pull_requests = [\"{PULL_REQUESTS}\", \"--since={{since}}\", \"--until={{until}}\"]\n"
    )
}

/// A stub source command first on PATH, recording the argv it was called with
/// so a test can say what pns asked for, and answering `body` for everything
/// else.
pub(super) fn stub_source(sandbox: &Sandbox, command: &mut std::process::Command, body: &str) {
    sandbox.stub_on_path(
        command,
        PULL_REQUESTS,
        &format!(
            "printf '%s\\n' \"$*\" >>\"{}/sources.argv\"\n{body}",
            sandbox.display()
        ),
    );
}

/// A review note written at `mtime`, under the sandbox's own home so a `~/`
/// glob reaches it.
pub(super) fn write_note(sandbox: &Sandbox, relative: &str, contents: &str, mtime: u64) {
    let path = sandbox.path(relative);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("the notes dir");
    std::fs::write(&path, contents).expect("the note");
    std::fs::File::options()
        .write(true)
        .open(&path)
        .expect("the note")
        .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(mtime))
        .expect("the note's clock");
}
