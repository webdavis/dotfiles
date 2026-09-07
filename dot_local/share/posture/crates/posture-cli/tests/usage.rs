//! Until a subcommand lands, the binary refuses EVERY word: usage on stderr,
//! nothing on stdout, exit 2 (spec S298 and S341: an unknown argument is an
//! error with usage and exit 2, never a silent fallthrough). The words below
//! are every planned subcommand from the specification's section 1 table, the
//! help spellings, and a word that will never exist.

use std::process::{Command, Output};

const WORDS: &[&[&str]] = &[
    &[],
    &["alert"],
    &["poll"],
    &["funnel"],
    &["watchdog"],
    &["digest"],
    &["heartbeat"],
    &["converge"],
    &["allowlist"],
    &["allowlist", "add", "com.example.agent"],
    &["allowlist", "deny", "com.example.agent"],
    &["allowlist", "list"],
    &["enrich", "/Applications/Safari.app"],
    &["ssh"],
    &["ssh", "install"],
    &["ssh", "verify"],
    &["ssh", "reload"],
    &["ssh", "rollback"],
    &["ssh", "print-config"],
    &["ssh", "print-path"],
    &["--help"],
    &["-h"],
    &["help"],
    &["frobnicate"],
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_posture"))
        .env_clear()
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .output()
        .expect("the posture binary runs")
}

#[test]
fn every_word_is_refused_with_usage_on_stderr_and_exit_2() {
    for args in WORDS {
        let output = run(args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "exit status for argv {args:?}"
        );
        assert!(
            output.stdout.is_empty(),
            "stdout must stay empty for argv {args:?}"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.starts_with("usage: posture "),
            "stderr must open with the usage line for argv {args:?}, got: {stderr}"
        );
    }
}

#[test]
fn the_usage_names_every_planned_subcommand() {
    let stderr = String::from_utf8_lossy(&run(&[]).stderr).into_owned();
    for phrase in [
        "alert |",
        "| poll |",
        "| funnel |",
        "| watchdog |",
        "| digest |",
        "| heartbeat |",
        "| converge",
        "allowlist add <label>",
        "allowlist deny <label>",
        "allowlist list",
        "enrich <path>",
        "ssh install|verify|reload|rollback|print-config|print-path",
    ] {
        assert!(
            stderr.contains(phrase),
            "usage must carry `{phrase}`, got: {stderr}"
        );
    }
}
