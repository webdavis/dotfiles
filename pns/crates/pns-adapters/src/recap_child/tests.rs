use super::*;
use std::time::{Duration, Instant};

mod fixture;
use fixture::{Fixture, child_fixture};

#[test]
fn the_whole_recap_deadline_includes_source_reads() {
    if child_fixture() {
        return;
    }
    let fixture = Fixture::new("whole-recap");
    let mut child = fixture.spawn("source-hang");
    fixture.ready();
    let status = fixture.wait(&mut child, Duration::from_millis(300));
    assert!(
        status.is_some_and(|status| !status.success()),
        "a blocked source read must be stopped by the whole recap deadline"
    );
}

#[test]
fn a_finished_recap_preserves_its_status_and_reaps_the_guardian() {
    if child_fixture() {
        return;
    }
    let fixture = Fixture::new("finished-recap");
    let mut child = fixture.spawn("complete");
    let status = fixture.wait(&mut child, Duration::from_millis(300));
    assert_eq!(status.and_then(|status| status.code()), Some(42));
}

#[test]
fn recap_setup_refusal_precedes_any_source_read() {
    let mut read = false;
    assert_eq!(
        recap_with_deadline(Instant::now() - Duration::from_millis(1), || {
            read = true;
            42
        }),
        1
    );
    assert!(
        !read,
        "no config, repository, or storage read may precede ownership"
    );
}
