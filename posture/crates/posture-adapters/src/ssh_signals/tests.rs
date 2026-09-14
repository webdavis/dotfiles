use super::*;
use std::{
    os::unix::process::ExitStatusExt,
    process::{Command, Stdio},
};

fn disposition(signal: i32) -> libc::sighandler_t {
    // A null action reads the current disposition without changing it.
    let mut current: libc::sigaction = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::sigaction(signal, std::ptr::null(), &mut current) },
        0
    );
    current.sa_sigaction
}

fn restore_default_dispositions() {
    // A disposition survives fork and exec, so a parent that ignores these (nohup, a launchd job)
    // decides what an inheriting fixture observes. Each fixture below arms real handlers and
    // raises real signals at itself, so it states its own starting dispositions rather than
    // taking whatever ran the suite.
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        assert_ne!(
            unsafe { libc::signal(signal, libc::SIG_DFL) },
            libc::SIG_ERR
        );
    }
}

#[test]
fn a_hangup_already_ignored_stays_ignored_and_never_cancels_the_install() {
    const MARKER: &str = "POSTURE_PRIVATE_SSH_IGNORED_HANGUP";
    if std::env::var_os(MARKER).is_some() {
        restore_default_dispositions();
        // This is nohup's disposition, inherited by every process it starts.
        assert_ne!(
            unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) },
            libc::SIG_ERR
        );
        let mut signals = SshSignals::arm().unwrap();
        assert_eq!(disposition(libc::SIGHUP), libc::SIG_IGN);
        // Only this separately spawned fixture process receives the signal.
        assert_eq!(unsafe { libc::raise(libc::SIGHUP) }, 0);
        assert_eq!(signals.pending(), None);
        assert!(!ssh_install_cancelled());
        // An interrupt carries no inherited SIG_IGN, so the skip is per signal.
        assert_eq!(unsafe { libc::raise(libc::SIGINT) }, 0);
        assert_eq!(signals.pending(), Some(libc::SIGINT));
        signals.disarm();
        assert_eq!(disposition(libc::SIGHUP), libc::SIG_IGN);
        println!("ignored-hangup-left-alone");
        return;
    }
    let run = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ssh_signals::tests::a_hangup_already_ignored_stays_ignored_and_never_cancels_the_install",
            "--nocapture",
        ])
        .env_clear()
        .env(MARKER, "1")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&run.stdout);
    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "{stdout}: {stderr}");
    assert!(
        stdout.contains("ignored-hangup-left-alone"),
        "{stdout}: {stderr}"
    );
}

#[test]
fn install_signals_are_deferred_through_rollback_then_reraised_as_real_signals() {
    const MARKER: &str = "POSTURE_PRIVATE_SSH_SIGNAL";
    if let Ok(signal) = std::env::var(MARKER) {
        restore_default_dispositions();
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
        // Every fixture path ends in a signal death or a panic, so waiting for its own exit is
        // the outcome itself and needs no clock.
        let run=Command::new(std::env::current_exe().unwrap()).args(["--exact","ssh_signals::tests::install_signals_are_deferred_through_rollback_then_reraised_as_real_signals","--nocapture"])
            .env_clear().env(MARKER,signal.to_string()).stdin(Stdio::null()).output().unwrap();
        let stdout = String::from_utf8_lossy(&run.stdout);
        let stderr = String::from_utf8_lossy(&run.stderr);
        assert_eq!(run.status.signal(), Some(signal), "{stderr}");
        assert!(
            stdout.contains("rollback-finished-before-reraise"),
            "{stdout}: {stderr}"
        );
    }
}
