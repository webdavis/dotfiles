//! `pns codex install-hooks`, through the real binary and against a HOME
//! inside the sandbox, so nothing here can reach the operator's own Codex.

mod support;

use support::{Sandbox, run, run_expecting, stderr};

fn hooks_file(sandbox: &Sandbox) -> std::path::PathBuf {
    sandbox.root.join(".codex").join("hooks.json")
}

#[test]
fn the_first_run_writes_the_hooks_and_the_second_says_nothing() {
    let sandbox = Sandbox::without_config("codex-install-hooks");
    let first = run(sandbox.pns().args(["codex", "install-hooks"]));
    assert_eq!(first.status.code(), Some(0), "{first:?}");
    assert!(
        stderr(&first).contains("/hooks"),
        "the first run must name the trust step: {first:?}"
    );
    let written = std::fs::read_to_string(hooks_file(&sandbox)).expect("the hooks file");
    for event in ["Stop", "PermissionRequest", "PostToolUse", "Interrupt"] {
        assert!(written.contains(event), "{event} is missing: {written}");
    }
    let second = run(sandbox.pns().args(["codex", "install-hooks"]));
    assert_eq!(second.status.code(), Some(0), "{second:?}");
    assert_eq!(stderr(&second), "", "an idempotent re-run stays silent");
    assert_eq!(
        std::fs::read_to_string(hooks_file(&sandbox)).expect("the hooks file"),
        written
    );
}

#[test]
fn a_hooks_file_this_cannot_read_is_refused_and_left_alone() {
    let sandbox = Sandbox::without_config("codex-install-hooks-refusal");
    let path = hooks_file(&sandbox);
    std::fs::create_dir_all(path.parent().expect("the codex directory")).expect("the directory");
    std::fs::write(&path, "{} {}\n").expect("the broken file");
    let output = run_expecting(2, sandbox.pns().args(["codex", "install-hooks"]));
    assert!(stderr(&output).contains("hooks.json"), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("the hooks file"),
        "{} {}\n"
    );
}
