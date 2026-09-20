use super::*;
use crate::current_uid;
use crate::test_sandbox::Sandbox;
use std::process::{Child, Command};

/// A child that is killed and reaped when the test drops it, so a failing
/// assertion never leaves a sleeping fixture behind.
struct Fixture(Child);
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Fixture {
    fn spawn(program: &Path) -> Self {
        Self(Command::new(program).arg("120").spawn().unwrap())
    }
    fn pid(&self) -> i32 {
        self.0.id() as i32
    }
}

fn table() -> LibprocProcesses {
    LibprocProcesses::default()
}

#[test]
fn a_spawned_fixture_is_found_by_its_name_real_user_and_parent() {
    let fixture = Fixture::spawn(Path::new("/bin/sleep"));
    let own_pid = std::process::id();
    // Containment, not equality: other tests in this binary spawn their own
    // `sleep` children, and an overlapping window can widen the match set.
    assert!(
        table()
            .matching("sleep", Some(current_uid()), Some(own_pid))
            .unwrap()
            .contains(&fixture.pid())
    );
    assert_eq!(
        table().matching("sleep", Some(current_uid() + 1), Some(own_pid)),
        Ok(vec![])
    );
    assert_eq!(
        table().matching("sleep", Some(current_uid()), Some(own_pid + 1)),
        Ok(vec![])
    );
}

#[test]
fn a_name_longer_than_a_record_name_field_is_still_matched_whole() {
    let sandbox = Sandbox::new("process-lookup");
    let name = "posture-fixture-with-a-name-far-longer-than-sixteen-bytes";
    let program = sandbox.path().join(name);
    std::fs::copy("/bin/sleep", &program).unwrap();
    let fixture = Fixture::spawn(&program);
    let own_pid = std::process::id();
    assert_eq!(
        table().matching(name, Some(current_uid()), Some(own_pid)),
        Ok(vec![fixture.pid()])
    );
    assert_eq!(
        table().matching(&name[..15], Some(current_uid()), Some(own_pid)),
        Ok(vec![])
    );
}

#[test]
fn a_name_nothing_runs_under_reads_as_absent_rather_than_unknown() {
    assert_eq!(
        table().matching("posture-nothing-is-called-this", None, None),
        Ok(vec![])
    );
}

#[test]
fn a_walk_that_does_not_answer_gives_the_deadline_back_to_the_caller() {
    let deadline = Duration::from_millis(50);
    let started = std::time::Instant::now();
    let answer = bounded_call(deadline, || {
        std::thread::sleep(Duration::from_secs(60));
        0
    });
    assert_eq!(answer, None);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert_eq!(
        LibprocProcesses::with_deadline(Duration::ZERO).matching("sleep", None, None),
        Err(InspectionFailure::TimedOut)
    );
}
