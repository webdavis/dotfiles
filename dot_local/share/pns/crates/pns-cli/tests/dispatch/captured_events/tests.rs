use super::{Sandbox, events, read_events};
use std::time::{Duration, Instant};

#[test]
fn an_unfinished_capture_gets_a_bounded_chance_to_finish() {
    let sandbox = Sandbox::new("capture-partial-record");
    std::fs::write(sandbox.path("hermes.events"), b"{\"state\":\"rec")
        .expect("the partial capture");
    let started = Instant::now();
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| events(&sandbox, "hermes")));
    assert!(
        outcome.is_err(),
        "an unfinished capture cannot be successful"
    );
    assert!(
        started.elapsed() >= Duration::from_millis(50),
        "an unfinished append was parsed before waiting for its delimiter"
    );
    assert!(started.elapsed() < Duration::from_millis(800));
}

#[test]
fn a_split_utf8_capture_is_never_reported_as_no_events() {
    let sandbox = Sandbox::new("capture-partial-utf8");
    std::fs::write(sandbox.path("hermes.events"), b"{\"detail\":\"\xc3")
        .expect("the partial UTF-8 capture");
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| events(&sandbox, "hermes")));
    assert!(
        outcome.is_err(),
        "a partial capture silently became no events"
    );
}

#[test]
fn a_completed_malformed_capture_still_fails() {
    let sandbox = Sandbox::new("capture-malformed-record");
    std::fs::write(sandbox.path("hermes.events"), b"{not json}\n")
        .expect("the completed malformed capture");
    let outcome =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| events(&sandbox, "hermes")));
    assert!(
        outcome.is_err(),
        "a completed malformed record was discarded"
    );
}

#[test]
fn complete_captures_keep_every_record_and_its_exact_values() {
    let sandbox = Sandbox::new("capture-complete-records");
    std::fs::write(
        sandbox.path("hermes.events"),
        "{\"state\":\"done\",\"detail\":\"first\"}\n{\"state\":\"recap\",\"detail\":\"é\"}\n",
    )
    .expect("the complete capture");
    assert_eq!(
        events(&sandbox, "hermes"),
        vec![
            serde_json::json!({"state": "done", "detail": "first"}),
            serde_json::json!({"state": "recap", "detail": "é"}),
        ]
    );
}

#[test]
fn a_split_write_finishes_without_losing_the_preceding_record() {
    use std::io::Write;
    use std::sync::mpsc;

    let sandbox = Sandbox::new("capture-split-write");
    let path = sandbox.path("hermes.events");
    let prefix =
        b"{\"state\":\"done\",\"detail\":\"first\"}\n{\"state\":\"recap\",\"detail\":\"\xc3";
    std::fs::write(&path, prefix).expect("the initial record and split UTF-8");
    let (finish, allowed) = mpsc::channel();
    let (completed, written) = mpsc::channel();
    std::thread::scope(|scope| {
        let writer_path = path.clone();
        let writer = scope.spawn(move || {
            allowed
                .recv_timeout(Duration::from_millis(250))
                .expect("the reader observed the unfinished write");
            std::fs::OpenOptions::new()
                .append(true)
                .open(&writer_path)
                .expect("the capture stays present")
                .write_all(b"\xa9\"}\n")
                .expect("the second record finishes");
            completed.send(()).expect("the reader waits for the writer");
        });
        let mut reads = 0;
        let recorded = read_events(
            || {
                let bytes = std::fs::read(&path)?;
                reads += 1;
                if reads == 1 {
                    assert_eq!(
                        bytes, prefix,
                        "the first snapshot is deliberately incomplete"
                    );
                    finish.send(()).expect("the writer is owned by this test");
                    written
                        .recv_timeout(Duration::from_millis(250))
                        .expect("the writer finished before the next read");
                }
                Ok(bytes)
            },
            "hermes",
        );
        writer.join().expect("the owned writer is reaped");
        assert_eq!(
            recorded,
            vec![
                serde_json::json!({"state": "done", "detail": "first"}),
                serde_json::json!({"state": "recap", "detail": "é"}),
            ]
        );
    });
}

#[test]
fn a_capture_read_error_cannot_claim_no_events() {
    let failure = std::panic::catch_unwind(|| {
        read_events(
            || Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            "hermes",
        )
    });
    assert!(
        failure.is_err(),
        "an unreadable capture became an empty success"
    );
}
