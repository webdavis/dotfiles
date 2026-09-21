//! The recap's summary section: when it is written, what stands in its place
//! when it is not, and what a pregenerated one is worth later.
//!
//! THE REAL BINARY IN A SANDBOX, with a SCRIPTED SUMMARIZER on its own PATH.
//! Never `claude`, `codex`, `ollama` or the live hermes: what these pin is the
//! wiring, and a test that reached a real model would pin the network.

mod support;

use support::{Sandbox, run, run_expecting, stdout};

/// One event in the store, `ago` seconds back.
fn planted(sandbox: &Sandbox, ago: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock")
        .as_secs();
    pns_adapters::SqliteStore::for_records(sandbox.state())
        .record_activity_event(&pns_domain::recap::activity::Event {
            at: now - ago,
            agent: "claude".into(),
            state: "blocked".into(),
            project: "dotfiles".into(),
            session: "one".into(),
            session_title: "the summarizer".into(),
            detail: "a decision is waiting".into(),
            ..pns_domain::recap::activity::Event::default()
        })
        .expect("the planted row");
    now - ago
}

/// A sandbox whose `[recap.summarizer]` is a script under its own directory.
fn sandbox_with(name: &str, body: &str) -> (Sandbox, std::path::PathBuf) {
    let sandbox = Sandbox::new(name);
    // SEVERAL SPAWNS OF THE REAL BINARY, each of which starts a scripted
    // summarizer of its own, so these sandboxes outlive the one-second
    // ceiling by design rather than by being slow.
    sandbox.allow_slow("several spawns of the engine and its summarizer");
    std::fs::create_dir_all(sandbox.state()).expect("the state dir");
    let script = sandbox.path("summarizer");
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("the script");
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .expect("the mode");
    sandbox.write_config(&format!(
        "[recap]\nminimum_events = 1\n[recap.summarizer]\ncommand = [\"{}\"]\ndeadline = \"10s\"\n",
        script.display()
    ));
    (sandbox, script)
}

/// A BARE TERMINAL RECAP ASKS NO MODEL, and `--summarize` is what asks.
#[test]
fn the_summary_is_written_only_when_it_is_asked_for_and_carries_its_own_time() {
    let (sandbox, _) = sandbox_with(
        "recap-summary-asked",
        "cat >/dev/null; printf 'one blocked session is waiting on you\\n'",
    );
    planted(&sandbox, 60);
    let quiet = stdout(&run(sandbox.pns_stateful().args(["recap", "today"])));
    assert!(
        !quiet.contains("SUMMARY"),
        "a bare recap asked a model: {quiet}"
    );
    let asked = stdout(&run(sandbox.pns_stateful().args([
        "recap",
        "today",
        "--summarize",
    ])));
    assert!(
        asked.contains("one blocked session is waiting on you"),
        "the paragraph is missing: {asked}"
    );
    assert!(
        asked.contains("SUMMARY") && asked.contains("(custom at "),
        "the summary names its source and the time it was written: {asked}"
    );
}

/// EVERY WAY OF NOT ANSWERING LEAVES ONE VISIBLE LINE, on the page and in the
/// document alike and with no `-v` asked for, and the mechanical sections are
/// untouched by it.
#[test]
fn a_summarizer_that_fails_leaves_one_visible_line_on_every_output_form() {
    for (name, body, expected) in [
        (
            "recap-summary-silent",
            "cat >/dev/null; exit 0",
            "answered nothing",
        ),
        (
            "recap-summary-refusing",
            "cat >/dev/null; echo nope >&2; exit 3",
            "gave no answer",
        ),
        (
            "recap-summary-hung",
            "cat >/dev/null; sleep 30",
            // THE TIMELINE'S OWN CALL SPENDS THE WHOLE EPISODE HANGING, so
            // this one finds no budget left rather than hanging a second
            // time: "answered nothing" is `Failure::Silent`'s wording for
            // both an empty answer and a spent budget.
            "answered nothing",
        ),
    ] {
        let (sandbox, script) = sandbox_with(name, body);
        if name.ends_with("hung") {
            // A DEADLINE SHORT ENOUGH TO PROVE EXPIRY WITHOUT WAITING, which
            // is what the config's one-millisecond floor exists for.
            sandbox.write_config(&format!(
                "[recap]\nminimum_events = 1\n[recap.summarizer]\ncommand = [\"{}\"]\n\
                 deadline = \"300ms\"\n",
                script.display()
            ));
        }
        planted(&sandbox, 60);
        let started = std::time::Instant::now();
        let page = stdout(&run(sandbox.pns_stateful().args([
            "recap",
            "today",
            "--summarize",
        ])));
        assert!(page.contains(expected), "{name}: {page}");
        assert!(
            page.contains("AGENTS"),
            "{name}: the sections went with it: {page}"
        );
        assert!(
            page.contains("OPEN"),
            "{name}: the summarizer took `open` with it: {page}"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{name}: the failure was waited on rather than bounded"
        );
        let document = stdout(&run(sandbox.pns_stateful().args([
            "recap",
            "today",
            "--summarize",
            "--json",
        ])));
        assert!(document.contains(expected), "{name}: {document}");
        assert!(
            document.contains("\"failed\":true"),
            "{name}: a consumer cannot tell a paragraph from an apology: {document}"
        );
    }
}

/// A PREGENERATED SUMMARY IS SHOWN LATER WITH THE TIME IT WAS WRITTEN, and is
/// written again only once the window has moved on.
#[test]
fn a_pregenerated_summary_is_stored_shown_and_regenerated_only_when_it_is_stale() {
    let (sandbox, _) = sandbox_with(
        "recap-summary-pregenerated",
        "cat >/dev/null; printf 'answer %s\\n' \"$(date +%s%N)\"",
    );
    planted(&sandbox, 600);
    run(sandbox
        .pns_stateful()
        .args(["recap", "today", "--pregenerate"]));
    let stored = pns_adapters::SqliteStore::for_records(sandbox.state())
        .recap_summary("today")
        .expect("the store")
        .expect("a stored summary");
    assert!(stored.text.starts_with("answer "), "{stored:?}");
    // A LATER READ SHOWS IT RATHER THAN ASKING AGAIN, which is what makes
    // pregenerating worth the model call.
    let shown = stdout(&run(sandbox
        .pns_stateful()
        .args(["recap", "today", "--json"])));
    assert!(shown.contains(&stored.text), "{shown}");
    let again = pns_adapters::SqliteStore::for_records(sandbox.state())
        .recap_summary("today")
        .expect("the store")
        .expect("a stored summary");
    assert_eq!(again.text, stored.text, "a fresh summary was rewritten");
    // AND A NEWER EVENT MAKES IT STALE, so the next pregenerate writes again.
    planted(&sandbox, 1);
    run(sandbox
        .pns_stateful()
        .args(["recap", "today", "--pregenerate"]));
    let rewritten = pns_adapters::SqliteStore::for_records(sandbox.state())
        .recap_summary("today")
        .expect("the store")
        .expect("a stored summary");
    assert_ne!(
        rewritten.text, stored.text,
        "a summary older than the window's last event was shown rather than written again"
    );
}

/// THE DOCTOR ROW RUNS THE REAL INVOCATION, which is the one check that
/// notices a harness changing its own flags.
#[test]
fn the_doctor_row_reports_the_configured_summarizers_presence() {
    let (sandbox, _) = sandbox_with("recap-summary-doctor", "cat >/dev/null; printf 'ok\\n'");
    // EXIT 1, because this sandbox reaches no phone and no ledger: what is
    // pinned here is the row, not the report's verdict.
    let printed = stdout(&run_expecting(1, sandbox.pns_stateful().arg("doctor")));
    assert!(
        printed.contains("the custom summarizer answered: ok"),
        "{printed}"
    );
    let (missing, _) = sandbox_with("recap-summary-doctor-missing", "exit 0");
    missing.write_config(
        "[recap]\nminimum_events = 1\n[recap.summarizer]\n\
         command = [\"pns-no-such-summarizer\"]\n",
    );
    let printed = stdout(&run_expecting(1, missing.pns_stateful().arg("doctor")));
    assert!(
        printed.contains("the custom summarizer could not be started"),
        "{printed}"
    );
}
