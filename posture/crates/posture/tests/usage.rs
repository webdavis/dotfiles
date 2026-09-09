//! Every remaining unimplemented subcommand is refused: usage on stderr,
//! nothing on stdout, exit 2 (spec S298 and S341: an unknown argument is an
//! error with usage and exit 2, never a silent fallthrough). The words below
//! are every planned subcommand from the specification's section 1 table, the
//! help spellings, and a word that will never exist.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const WORDS: &[&[&str]] = &[
    &[],
    &["alert"],
    &["poll"],
    &["funnel"],
    &["watchdog"],
    &["allowlist"],
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

fn run(args: &[&str], deadline: Instant) -> Output {
    run_with_stderr(args, deadline, Stdio::piped())
}

fn run_with_stderr(args: &[&str], deadline: Instant, stderr: Stdio) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_posture"))
        .env_clear()
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .expect("the posture binary runs");
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .expect("posture output is readable");
            }
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            state => {
                let killed = child.kill();
                let reaped = child.wait();
                panic!(
                    "posture argv {args:?} failed to finish within the 500 ms test deadline: \
                     poll: {state:?}; termination: {killed:?}; reap: {reaped:?}"
                );
            }
        }
    }
}

#[test]
fn every_unimplemented_word_is_refused_with_usage_on_stderr_and_exit_2() {
    let deadline = Instant::now() + Duration::from_millis(500);
    for args in WORDS {
        let output = run(args, deadline);
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
    let deadline = Instant::now() + Duration::from_millis(500);
    let stderr = String::from_utf8_lossy(&run(&[], deadline).stderr).into_owned();
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

#[test]
fn a_closed_stderr_reader_preserves_the_refusal_exit_code() {
    let (reader, writer) = UnixStream::pair().expect("the fixture socket pair opens");
    drop(reader);
    let output = run_with_stderr(
        &["alert"],
        Instant::now() + Duration::from_millis(500),
        Stdio::from(OwnedFd::from(writer)),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

#[test]
fn enrich_with_an_absent_or_empty_path_is_successful_and_silent() {
    for args in [
        vec!["enrich"],
        vec!["enrich", ""],
        vec!["enrich", "", "ignored.app"],
    ] {
        let output = run(&args, Instant::now() + Duration::from_millis(500));
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn enrich_inspects_a_private_non_code_file_and_ignores_trailing_operands() {
    let directory = std::env::temp_dir().join(format!("posture-metadata-{}", std::process::id()));
    std::fs::create_dir(&directory).expect("private fixture directory");
    let path = directory.join("file with spaces");
    std::fs::write(&path, b"inert non-code fixture").expect("fixture contents");
    let output = run(
        &[
            "enrich",
            path.to_str().expect("fixture path"),
            "ignored.app",
        ],
        Instant::now() + Duration::from_millis(500),
    );
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).expect("ASCII metadata");
    assert!(text.starts_with("owner "), "{text}");
    assert!(text.contains(", mode -rw"), "{text}");
    assert!(text.contains(", modified "), "{text}");
    assert!(text.ends_with('Z'), "no added newline: {text}");
}
