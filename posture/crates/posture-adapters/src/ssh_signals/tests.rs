use super::*;
use std::{
    io::Read,
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn install_signals_are_deferred_through_rollback_then_reraised_as_real_signals() {
    const MARKER: &str = "POSTURE_PRIVATE_SSH_SIGNAL";
    if let Ok(signal) = std::env::var(MARKER) {
        let signal: i32 = signal.parse().unwrap();
        assert!([libc::SIGINT, libc::SIGTERM, libc::SIGHUP].contains(&signal));
        let mut signals = SshSignals::arm().unwrap();
        assert!(SshSignals::arm().is_err());
        // Only this separately spawned fixture process receives the signal.
        assert_eq!(unsafe { libc::raise(signal) }, 0);
        assert_eq!(signals.pending(), Some(signal));
        assert!(ssh_install_cancelled());
        signals.defer();
        assert_eq!(signals.pending(), Some(signal));
        assert!(!ssh_install_cancelled());
        // A second signal during rollback must not recursively kill this fixture.
        assert_eq!(unsafe { libc::raise(libc::SIGTERM) }, 0);
        assert_eq!(signals.pending(), Some(signal));
        assert!(!ssh_install_cancelled());
        println!("rollback-finished-before-reraise");
        signals.reraise();
        panic!("the original signal must terminate this fixture");
    }
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        let mut child=Command::new(std::env::current_exe().unwrap()).args(["--exact","ssh_signals::tests::install_signals_are_deferred_through_rollback_then_reraised_as_real_signals","--nocapture"])
            .env_clear().env(MARKER,signal.to_string()).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_millis(250);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("private signal child exceeded 250ms");
            }
            std::thread::sleep(Duration::from_millis(1));
        };
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .unwrap()
            .read_to_string(&mut stdout)
            .unwrap();
        let mut stderr = String::new();
        child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        assert_eq!(status.signal(), Some(signal), "{stderr}");
        assert!(
            stdout.contains("rollback-finished-before-reraise"),
            "{stdout}: {stderr}"
        );
    }
}
