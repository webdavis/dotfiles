use super::*;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    let home = std::env::temp_dir().join(format!(
        "uu-schedule-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&home).expect("owned runtime home");
    home
}

fn executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("owned executable parent")).expect("runtime directory");
    fs::write(path, text).expect("owned executable");
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("executable permission");
}

fn rendered_path(home: &Path) -> String {
    let plist = render_plist(
        DEFAULT_LABEL,
        home.to_str().expect("fixture path"),
        Schedule::default(),
    );
    let mut child = Command::new("/usr/bin/plutil")
        .args([
            "-extract",
            "EnvironmentVariables.PATH",
            "raw",
            "-o",
            "-",
            "-",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("system property-list parser");
    child
        .stdin
        .take()
        .expect("parser stdin")
        .write_all(plist.as_bytes())
        .expect("rendered input");
    let output = child
        .wait_with_output()
        .expect("parse rendered environment");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("rendered path")
        .trim_end()
        .into()
}

fn run(home: &Path, script: &Path) -> std::process::Output {
    Command::new(script)
        .env("PATH", rendered_path(home))
        .env("HOME", home)
        .output()
        .expect("owned rendered-path child")
}

#[test]
fn a_rendered_job_finds_an_interpreter_available_only_in_the_homes_fnm_directory() {
    let home = fixture();
    executable(
        &home.join(".local/share/fnm/aliases/default/bin/uu-owned-node"),
        "#!/bin/sh\nprintf 'owned node runtime'\n",
    );
    let script = home.join("owned-package");
    executable(&script, "#!/usr/bin/env uu-owned-node\n");
    let output = run(&home, &script);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"owned node runtime");
}

#[test]
fn a_rendered_job_finds_an_interpreter_available_only_in_the_homes_cargo_directory() {
    let home = fixture();
    executable(
        &home.join(".cargo/bin/uu-owned-cargo-tool"),
        "#!/bin/sh\nprintf 'owned cargo runtime'\n",
    );
    let script = home.join("owned-package");
    executable(&script, "#!/usr/bin/env uu-owned-cargo-tool\n");
    let output = run(&home, &script);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"owned cargo runtime");
}

#[test]
fn a_rendered_job_uses_the_homes_fnm_interpreter_before_the_system_interpreter() {
    let home = fixture();
    executable(
        &home.join(".local/share/fnm/aliases/default/bin/sh"),
        "#!/bin/sh\nprintf 'owned fnm runtime'\n",
    );
    let script = home.join("owned-package");
    executable(
        &script,
        "#!/usr/bin/env sh\nprintf 'system interpreter read the owned body'\n",
    );
    let output = run(&home, &script);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"owned fnm runtime");
}
