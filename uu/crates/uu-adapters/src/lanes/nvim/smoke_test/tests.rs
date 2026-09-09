use super::*;
use crate::config::NvimHost;
use crate::lanes::{Ran, Verdict};
use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::PathBuf;
use std::time::Duration;
use uu_domain::LaneVerdict;
mod fixture;
use fixture::{Child, Fixture};

#[test]
fn the_smoke_test_runs_nvim_through_env_with_the_four_base_directories_redirected() {
    let f = Fixture::new("args");
    let child = Child::new(&f, "ok");
    assert_eq!(f.run(&child).verdict(), LaneVerdict::Completed);
    let calls = child.calls.borrow();
    for call in calls.iter() {
        assert_eq!(call[0], "/usr/bin/env");
        for (key, leaf) in [
            ("XDG_CONFIG_HOME", "c"),
            ("XDG_DATA_HOME", "d"),
            ("XDG_STATE_HOME", "s"),
            ("XDG_CACHE_HOME", "k"),
        ] {
            assert!(
                call.contains(&format!("{key}={}/{leaf}", f.lane.cache)),
                "{call:?}"
            );
        }
        let nvim = call.iter().position(|a| a == "/fixture/nvim").unwrap();
        assert_eq!(
            &call[nvim + 1..nvim + 4],
            &[
                "--headless",
                "-u",
                &format!("{}/c/nvim/init.lua", f.lane.cache)
            ]
        );
    }
}

#[test]
fn both_smoke_children_keep_home_and_claude_discovery_private() {
    let f = Fixture::new("home");
    let child = Child::new(&f, "ok");
    f.run(&child);
    let calls = child.calls.borrow();
    assert_eq!(calls.len(), 2);
    for call in calls.iter() {
        assert!(
            call.contains(&format!("HOME={}/h", f.lane.cache)),
            "{call:?}"
        );
        assert!(
            call.contains(&format!("CLAUDE_CONFIG_DIR={}/h/.claude", f.lane.cache)),
            "{call:?}"
        );
    }
    for leaf in ["h", "h/.claude"] {
        assert_eq!(
            fs::metadata(f.cache.join(leaf))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[test]
fn the_smoke_test_copies_config_and_mason_without_linking_live_state() {
    let f = Fixture::new("copy");
    let child = Child::new(&f, "ok");
    f.run(&child);
    assert_eq!(
        fs::read_to_string(f.cache.join("c/nvim/init.lua")).unwrap(),
        "owned init"
    );
    let tool = f.cache.join("d/nvim/mason/bin/owned");
    assert_eq!(fs::read_to_string(&tool).unwrap(), "owned tool");
    assert!(
        !fs::symlink_metadata(&tool)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    fs::write(tool, "candidate change").unwrap();
    assert_eq!(
        fs::read_to_string(f.data.join("nvim/mason/package/tool")).unwrap(),
        "owned tool"
    );
}

#[test]
fn candidate_verification_starts_a_new_process_after_prepare_succeeds() {
    let f = Fixture::new("fresh");
    let child = Child::new(&f, "ok");
    f.run(&child);
    let calls = child.calls.borrow();
    assert_eq!(calls.len(), 2);
    assert_eq!(
        &calls[0][calls[0].len() - 3..],
        &[
            "-l",
            &format!("{}/c/nvim/lua/uu/smoke_test.lua", f.lane.cache),
            "prepare"
        ]
    );
    assert_eq!(
        &calls[1][calls[1].len() - 2..],
        &[
            "-c",
            &format!(
                "lua dofile('{}/c/nvim/lua/uu/smoke_test.lua')",
                f.lane.cache
            )
        ]
    );
}

#[test]
fn a_failed_prepare_never_launches_the_verifier() {
    let f = Fixture::new("prepare");
    let child = Child::new(&f, "prepare-fail");
    let report = f.run(&child);
    assert_eq!(child.calls.borrow().len(), 1);
    assert_eq!(report.failures(), 1);
    assert!(
        report.lines.iter().any(|s| s.contains("prepare failed")),
        "{:?}",
        report.lines
    );
}

#[test]
fn a_missing_completion_or_unreadable_loaded_notifier_history_fails_verification() {
    for mode in ["missing", "unreadable", "stale", "lock-mismatch"] {
        let f = Fixture::new(mode);
        let child = Child::new(&f, mode);
        let report = f.run(&child);
        assert_eq!(child.calls.borrow().len(), 2);
        assert_eq!(report.failures(), 1, "{mode}: {:?}", report.lines);
        assert!(
            report
                .lines
                .iter()
                .any(|s| s.contains("completion") || s.contains("history")),
            "{mode}: {:?}",
            report.lines
        );
    }
}

#[test]
fn a_smoke_test_child_exiting_non_zero_is_a_counted_failure() {
    let f = Fixture::new("exit");
    let child = Child::new(&f, "verify-fail");
    let report = f.run(&child);
    assert_eq!(report.failures(), 1);
    assert!(
        report.lines.iter().any(|s| s.contains("verify failed")),
        "{:?}",
        report.lines
    );
}

#[test]
fn verifier_stderr_fails_even_after_a_successful_exit_and_valid_completion() {
    let f = Fixture::new("stderr");
    let child = Child::new(&f, "stderr");
    let report = f.run(&child);
    assert_eq!(report.failures(), 1);
    assert!(
        report.lines.iter().any(|s| s.contains("startup stderr")),
        "{:?}",
        report.lines
    );
    assert_eq!(
        fs::read_to_string(f.cache.join("startup.stderr")).unwrap(),
        "startup stderr"
    );
}

mod isolation;
