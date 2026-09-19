use super::*;

/// A config whose every source is present and answerable without a network.
fn full_config(directory: &Path) -> String {
    let apply_log = directory.join("apply.log");
    std::fs::write(
        &apply_log,
        "started:    2026-09-17T10:17:26Z\nfinished:   2026-09-17T10:18:49Z\nexit_code:  0\nresult:     OK\n",
    )
    .unwrap();
    let ledger = directory.join("remaining-work.md");
    std::fs::write(
        &ledger,
        "- [ ] 12. Ship it; the operator owes a `chezmoi apply`.\n- [ ] 13. Nothing owed here.\n",
    )
    .unwrap();
    let recaps = directory.join("recaps");
    std::fs::create_dir_all(&recaps).unwrap();
    std::fs::write(recaps.join("2026-09-17.md"), "two PRs merged overnight\n").unwrap();
    format!(
        "[apply_log]\npath = {apply_log:?}\n\n\
         [ledger]\npath = {ledger:?}\n\n\
         [recap]\ndirectory = {recaps:?}\n\n\
         [pull_requests]\ncommand = [\"/bin/echo\", \"#781 green\"]\n\n\
         [tasks]\ncommand = [\"/bin/echo\", \"call the clinic\"]\n"
    )
}

fn run_with(config: &str, directory: &Path) -> Response {
    let path = directory.join("config.toml");
    std::fs::write(&path, config).unwrap();
    run(&[], &path, Path::new("/nonexistent-home"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!("morning-test-{name}"));
    std::fs::remove_dir_all(&directory).ok();
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn a_full_page_carries_every_section_with_what_its_source_reported() {
    let directory = scratch("full");
    let response = run_with(&full_config(&directory), &directory);
    assert_eq!(response.exit, 0);
    assert_eq!(response.stderr, "");
    let page = response.stdout;
    assert!(page.starts_with("\nmorning\n"), "{page}");
    assert!(
        page.contains("◆ Last apply ──\n  OK at 2026-09-17T10:18:49Z\n"),
        "{page}"
    );
    assert!(page.contains("◆ Applies owed ──\n  12. Ship it;"), "{page}");
    assert!(
        page.contains("◆ Pull requests ──\n  #781 green\n"),
        "{page}"
    );
    assert!(
        page.contains("◆ Overnight recap ──\n  two PRs merged overnight\n"),
        "{page}"
    );
    assert!(
        page.contains("◆ Operator's own items ──\n  12. Ship it;"),
        "{page}"
    );
    assert!(page.contains("◆ Today ──\n  call the clinic\n"), "{page}");
}

#[test]
fn an_empty_config_still_prints_every_section_and_says_none_is_configured() {
    let directory = scratch("empty");
    let response = run_with("", &directory);
    assert_eq!(response.exit, 0);
    let unconfigured = response
        .stdout
        .matches("unavailable: not configured")
        .count();
    assert_eq!(unconfigured, 6, "{}", response.stdout);
}

#[test]
fn a_source_whose_command_fails_reports_the_failure_in_its_own_section() {
    let directory = scratch("failing");
    let config =
        "[tasks]\ncommand = [\"/bin/sh\", \"-c\", \"echo 'td: not installed' >&2; exit 3\"]\n";
    let page = run_with(config, &directory).stdout;
    assert!(
        page.contains("◆ Today ──\n  unavailable: it exited 3: td: not installed\n"),
        "{page}"
    );
}

#[test]
fn a_source_that_hangs_is_abandoned_and_the_rest_of_the_page_still_prints() {
    let directory = scratch("slow");
    let config = "timeout_seconds = 0.2\n\n\
        [pull_requests]\ncommand = [\"/bin/sh\", \"-c\", \"sleep 30\"]\n\n\
        [tasks]\ncommand = [\"/bin/echo\", \"call the clinic\"]\n";
    let started = std::time::Instant::now();
    let page = run_with(config, &directory).stdout;
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "{:?}",
        started.elapsed()
    );
    assert!(
        page.contains("◆ Pull requests ──\n  unavailable: no answer within 0.2 seconds"),
        "{page}"
    );
    assert!(page.contains("◆ Today ──\n  call the clinic\n"), "{page}");
}

#[test]
fn no_color_is_a_known_flag_in_any_position() {
    let directory = scratch("no-color");
    let path = directory.join("config.toml");
    std::fs::write(&path, "").unwrap();
    for args in [
        vec!["--no-color".to_string()],
        vec![
            "--config".to_string(),
            path.display().to_string(),
            "--no-color".to_string(),
        ],
    ] {
        let response = run(&args, &path, Path::new("/nonexistent-home"));
        assert_eq!(response.exit, 0, "{:?}: {}", args, response.stderr);
    }
}

#[test]
fn an_unknown_argument_is_refused_rather_than_printing_a_page() {
    let response = run(
        &["--apply".to_string()],
        Path::new("/nonexistent/config.toml"),
        Path::new("/nonexistent-home"),
    );
    assert_eq!(response.exit, 2);
    assert!(
        response
            .stderr
            .starts_with("morning: unknown argument --apply"),
        "{}",
        response.stderr
    );
    assert_eq!(response.stdout, "");
}

#[test]
fn a_broken_config_names_the_file_rather_than_printing_half_a_page() {
    let directory = scratch("broken");
    let response = run_with("timeout_seconds = [\n", &directory);
    assert_eq!(response.exit, 1);
    assert!(
        response.stderr.contains("config.toml"),
        "{}",
        response.stderr
    );
}
