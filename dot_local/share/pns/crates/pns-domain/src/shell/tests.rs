use super::*;

#[test]
fn every_interactive_tui_on_the_skip_list_publishes_no_marker() {
    for name in [
        "vim", "nvim", "less", "man", "top", "btop", "ssh", "herdr", "claude", "hermes", "codex",
        "fzf",
    ] {
        for line in [
            name.to_string(),
            format!("{name} src/main.rs"),
            format!("{name}\tfile"),
        ] {
            assert!(shell_is_interactive(&line), "{line:?}");
            assert!(shell_event(&line, 0, 300, String::new(), String::new()).is_none());
        }
    }
}

#[test]
fn a_build_whose_name_merely_starts_with_a_tuis_is_still_a_build() {
    for line in [
        "topaz build",
        "manage.py migrate",
        "lessc styles.less",
        "sshuttle -r host",
        "codexify run",
        "true; nvim",
        " nvim",
    ] {
        assert!(!shell_is_interactive(line), "{line:?}");
        assert!(shell_event(line, 0, 300, String::new(), String::new()).is_some());
    }
    assert!(shell_is_interactive("nvim --version; sleep 60"));
}

#[test]
fn shell_tiers_keep_both_boundaries_and_their_neighbors() {
    for elapsed in [0, 29, 30, 31, 299, 300, 301] {
        let event = shell_event("cargo build", 0, elapsed, "project".into(), "t1:p2".into());
        if elapsed < 30 {
            assert!(event.is_none());
            continue;
        }
        let event = event.unwrap();
        assert_eq!(event.long_running, elapsed >= 300);
        assert_eq!(event.detail, format!("cargo ({elapsed}s)"));
        assert_eq!(event.agent, "shell");
        assert_eq!(event.state, "done");
        assert_eq!(event.project, "project");
        assert_eq!(event.pane, "t1:p2");
        assert!(!event.local_only && !event.remote_only);
    }
}

#[test]
fn a_failed_command_names_its_status_without_exposing_arguments() {
    let event = shell_event(
        "deploy --password invisible",
        137,
        301,
        String::new(),
        String::new(),
    )
    .unwrap();
    assert_eq!(event.detail, "deploy (301s, exit 137)");
    assert_eq!(event.state, "failed");
    assert!(event.long_running);
}

#[test]
fn command_display_keeps_the_legacy_literal_space_boundary() {
    for (line, expected) in [
        ("", " (30s)"),
        (" leading", " (30s)"),
        ("cargo\tbuild arg", "cargo\tbuild (30s)"),
    ] {
        assert_eq!(
            shell_event(line, 0, 30, String::new(), String::new())
                .unwrap()
                .detail,
            expected
        );
    }
}
