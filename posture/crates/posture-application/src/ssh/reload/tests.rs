use super::*;
use crate::{InspectionFailure, SshCompleted, SshScanFailure};
use posture_domain::SshTreeRefusal;
use std::collections::VecDeque;
mod fixtures;
use fixtures::*;

#[test]
fn reload_orders_all_proofs_and_flushes_recovery_before_restart() {
    let mut f = Fixture::new();
    let (status, out, err) = f.run(ready(), false);
    assert_eq!(status, 0, "{err}");
    assert_eq!(
        &f.trace.borrow()[..13],
        &[
            "prime",
            "prover",
            "observe",
            "syntax",
            "verify",
            "ports",
            "probe",
            "observe",
            "warning-flush",
            "restart",
            "probe",
            "banner:22:1",
            "observe"
        ]
    );
    assert!(out.contains("posture ssh rollback"));
    assert!(out.contains("Keep this SSH session OPEN"));
    assert!(out.contains("not a guarantee"));
}
#[test]
fn invalid_readiness_does_nothing_and_unavailable_privilege_or_prover_never_reads_tree() {
    let mut f = Fixture::new();
    let (status, _, err) = f.run(SshReadiness::parse("0", "1", "1"), false);
    assert_eq!(status, 1);
    assert!(!err.is_empty());
    assert!(f.trace.borrow().is_empty());
    let mut f = Fixture::new();
    f.prime = Err(InspectionFailure::Failed);
    let (status, _, err) = f.run(ready(), false);
    assert_eq!(status, 1);
    assert!(err.contains("privilege"));
    assert_eq!(*f.trace.borrow(), ["prime"]);
    let mut f = Fixture::new();
    f.available = false;
    let (status, _, err) = f.run(ready(), false);
    assert_eq!(status, 1);
    assert!(err.contains("prover"));
    assert_eq!(*f.trace.borrow(), ["prime", "prover"]);
}
#[test]
fn initial_tree_syntax_verification_and_port_failures_stop_before_service_probe() {
    for step in 0..4 {
        let mut f = Fixture::new();
        match step {
            0 => f.trees.borrow_mut()[0] = Err(SshScanFailure::File(SshTreeRefusal::Bytes)),
            1 => f.syntax = Err(InspectionFailure::TimedOut),
            2 => f.verification = SshVerification::Failed(vec!["bad policy".into()]),
            _ => f.ports = good("port 022\n"),
        }
        let (status, _, err) = f.run(ready(), false);
        assert_eq!(status, 1);
        assert!(err.contains("not touched"), "{err}");
        assert!(!f.trace.borrow().contains(&"probe".into()));
    }
}
#[test]
fn only_exact_service_absence_is_a_noop_and_changed_absent_tree_downgrades_the_claim() {
    for change in [false, true] {
        let mut f = Fixture::new();
        f.loaded[0] = Ok(SshCompleted {
            status: 113,
            output: vec![],
        });
        if change {
            f.trees.borrow_mut()[1] = Ok(record(1));
        }
        let (status, out, err) = f.run(ready(), false);
        assert_eq!(status, 0, "{err}");
        assert!(!f.trace.borrow().contains(&"restart".into()));
        if change {
            assert!(err.contains("CHANGED"));
            assert!(!out.contains("next start"));
        } else {
            assert!(out.contains("next start"));
        }
    }
    for status in [1, 127, 255] {
        let mut f = Fixture::new();
        f.loaded[0] = Ok(SshCompleted {
            status,
            output: b"probe error".to_vec(),
        });
        let (exit, _, err) = f.run(ready(), false);
        assert_eq!(exit, 1);
        assert!(err.contains("probe error"));
        assert!(!f.trace.borrow().contains(&"restart".into()));
    }
}
#[test]
fn unreadable_absent_tree_and_changed_or_unreadable_loaded_tree_refuse_without_restart() {
    for absent in [false, true] {
        let mut f = Fixture::new();
        if absent {
            f.loaded[0] = Ok(SshCompleted {
                status: 113,
                output: vec![],
            });
        }
        f.trees.borrow_mut()[1] = Err(SshScanFailure::File(SshTreeRefusal::Empty));
        let (status, _, err) = f.run(ready(), false);
        assert_eq!(status, 1);
        assert!(err.contains("read"));
        assert!(!f.trace.borrow().contains(&"restart".into()));
    }
    let mut f = Fixture::new();
    f.trees.borrow_mut()[1] = Ok(record(1));
    let (status, _, err) = f.run(ready(), false);
    assert_eq!(status, 1);
    assert!(err.contains("CHANGED"));
    assert!(!f.trace.borrow().contains(&"restart".into()));
}
#[test]
fn a_failed_warning_flush_prevents_the_disruptive_call() {
    let mut f = Fixture::new();
    let (status, _, _) = f.run(ready(), true);
    assert_eq!(status, 1);
    assert!(f.trace.borrow().contains(&"warning-flush".into()));
    assert!(!f.trace.borrow().contains(&"restart".into()));
}
#[test]
fn failed_restart_or_post_restart_service_probe_repeats_recovery_and_never_rolls_back() {
    for probe in [false, true] {
        let mut f = Fixture::new();
        if probe {
            f.loaded[1] = Err(InspectionFailure::TimedOut);
        } else {
            f.restart = Err(InspectionFailure::TimedOut);
        }
        let (status, _, err) = f.run(ready(), false);
        assert_eq!(status, 1);
        assert!(err.contains("posture ssh rollback"));
        assert!(err.contains("Remote Login"));
        assert!(f.trace.borrow().contains(&"restart".into()));
        assert!(!f.trace.borrow().iter().any(|s| s.starts_with("banner:")));
    }
}
#[test]
fn readiness_requires_zero_status_and_a_key_record_and_retries_all_ports() {
    let mut f = Fixture::new();
    f.banners = VecDeque::from([
        good("# comment\n"),
        Ok(SshCompleted {
            status: 1,
            output: b"host type key".to_vec(),
        }),
        good(""),
        good("host type key"),
    ]);
    let (status, out, err) = f.run(ready(), false);
    assert_eq!(status, 0, "{err}");
    assert!(out.contains("port 2222"));
    let calls: Vec<_> = f
        .trace
        .borrow()
        .iter()
        .filter(|s| s.starts_with("banner:") || s.starts_with("pause:"))
        .cloned()
        .collect();
    assert_eq!(
        calls,
        [
            "banner:22:1",
            "banner:2222:1",
            "pause:5",
            "banner:22:1",
            "banner:2222:1"
        ]
    );
}
#[test]
fn silent_listener_is_reported_before_post_restart_drift_with_launchd_socket_caveat() {
    let mut f = Fixture::new();
    f.banners.clear();
    f.trees.borrow_mut()[2] = Ok(record(2));
    let (status, _, err) = f.run(ready(), false);
    assert_eq!(status, 1);
    assert!(err.contains("POSSIBLE LOCKOUT"));
    assert!(err.contains("normally 22"));
    assert!(err.contains("posture ssh rollback"));
    assert_eq!(
        f.trace
            .borrow()
            .iter()
            .filter(|s| s.as_str() == "observe")
            .count(),
        2
    );
}
#[test]
fn post_banner_drift_or_read_error_refuses_a_success_claim_without_automatic_rollback() {
    for unreadable in [false, true] {
        let mut f = Fixture::new();
        f.trees.borrow_mut()[2] = if unreadable {
            Err(SshScanFailure::File(SshTreeRefusal::Empty))
        } else {
            Ok(record(1))
        };
        let (status, out, err) = f.run(ready(), false);
        assert_eq!(status, 1);
        assert!(err.contains("RESTARTED and answered"));
        assert!(err.contains("Nothing was rolled back"));
        assert!(!out.contains("reload complete"));
    }
}
