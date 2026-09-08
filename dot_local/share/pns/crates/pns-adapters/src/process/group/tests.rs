use super::*;
use std::process::Stdio;
use std::time::Duration;

#[test]
fn the_owner_pipe_cannot_be_inherited_by_an_executed_command() {
    let group = Group::start(Instant::now() + Duration::from_millis(300)).unwrap();
    let fd = group.owner.as_ref().unwrap().as_raw_fd();
    // SAFETY: F_GETFD observes our still-open owner descriptor.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
}

#[test]
fn a_completed_exit_42_is_preserved_and_its_cleanup_child_is_reaped() {
    let group = Group::start(Instant::now() + Duration::from_millis(300)).unwrap();
    let pid = group.pid;
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exit 42"]);
    let status = group.spawn(&mut command).unwrap().wait().unwrap();
    drop(group);
    assert_eq!(status.code(), Some(42));
    // SAFETY: waitpid can only reap this test's own known cleanup child.
    let waited = unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
    assert_eq!(
        waited, -1,
        "Drop must already have reaped its cleanup child"
    );
    assert_eq!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::ECHILD)
    );
}

#[test]
fn failed_cleanup_setup_refuses_before_the_command_can_have_side_effects() {
    let (mut reader, writer) = io::pipe().unwrap();
    let mut command = Command::new("/usr/bin/true");
    command.stdin(Stdio::null()).stdout(Stdio::null());
    // SAFETY: the child callback only writes a fixed byte to its inherited pipe.
    // It marks the first possible command-side effect, before exec itself.
    unsafe {
        command.pre_exec(move || {
            libc::write(writer.as_raw_fd(), [1u8].as_ptr().cast(), 1);
            Ok(())
        });
    }
    let result = super::super::bounded::finish_bounded(
        &mut command,
        None,
        Instant::now() - Duration::from_millis(1),
        0,
    );
    drop(command);
    let mut effects = Vec::new();
    reader.read_to_end(&mut effects).unwrap();
    assert!(result.is_err());
    assert!(
        effects.is_empty(),
        "cleanup refusal must precede command spawn"
    );
}

#[test]
fn the_cleanup_deadline_holds_while_the_producer_still_owns_its_pipe() {
    let group = Group::start(Instant::now() + Duration::from_millis(80)).unwrap();
    let mut command = Command::new("/bin/sleep");
    command.arg("10").stdin(Stdio::null()).stdout(Stdio::null());
    let mut child = group.spawn(&mut command).unwrap();
    let status = super::super::wait::wait_until(
        &mut child,
        Instant::now() + Duration::from_millis(250),
        std::thread::sleep,
    );
    drop(group);
    let _ = child.wait();
    assert!(status.is_some_and(|status| !status.success()));
}

#[test]
fn killing_a_descendant_at_the_deadline_does_not_turn_partial_output_into_an_answer() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf partial; sleep 10 & exit 0"]);
    assert_eq!(
        super::super::bounded::run_bounded(command, None, Duration::from_millis(80), 128),
        None,
    );
}

mod lifecycle;
