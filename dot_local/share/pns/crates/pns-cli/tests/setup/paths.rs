use super::*;

#[test]
fn a_dangling_symlink_at_the_config_path_is_refused_before_the_first_question() {
    // NO PTY NEEDED: the config check runs before the tty check, so a plain
    // pipe (here, `/dev/null`) is enough to tell which one fired first.
    let sandbox = Sandbox::without_config("setup-dangling-symlink");
    let config_dir = sandbox.root.join(".config/pns");
    std::fs::create_dir_all(&config_dir).expect("the config directory");
    let config_path = config_dir.join("config.toml");
    std::os::unix::fs::symlink(config_dir.join("nowhere.toml"), &config_path)
        .expect("the dangling symlink");

    let output = sandbox
        .bare()
        .args(["setup"])
        .stdin(std::process::Stdio::null())
        .output()
        .expect("the wizard runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists"),
        "the dangling symlink was not caught by the pre-check: {stderr}"
    );
    assert!(
        !stderr.contains("not a terminal"),
        "the pre-check ran after the tty check instead of before it: {stderr}"
    );
}

#[test]
fn a_dangling_link_above_the_config_is_refused_before_the_first_question() {
    // THE LEAF REPORTS `NotFound` WITHOUT BEING ABSENT: resolving
    // `config.toml` walks through `pns`, which resolves to nothing, so the
    // stat fails with ENOENT exactly the way a genuinely missing config
    // does. Reading that as absence walks all ten questions and only then
    // fails to publish, with every answer already typed and every secret
    // already handed over.
    for arguments in [vec!["setup"], vec!["setup", "--force"]] {
        let sandbox = Sandbox::without_config("setup-dangling-parent");
        let config_root = sandbox.root.join(".config");
        std::fs::create_dir_all(&config_root).expect("the config root");
        let link = config_root.join("pns");
        std::os::unix::fs::symlink(config_root.join("nowhere"), &link)
            .expect("the dangling parent link");

        let output = sandbox
            .bare()
            .args(&arguments)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("the wizard runs");

        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(&link.display().to_string()),
            "the refusal does not name the link that resolves to nothing \
             ({arguments:?}): {stderr}"
        );
        // ORDERING, AND THAT `--force` CANNOT BUY PAST IT: reaching the tty
        // check means the pre-check took the dangling link for absence.
        assert!(
            !stderr.contains("not a terminal"),
            "the dangling parent was accepted as absence ({arguments:?}): {stderr}"
        );
    }
}

#[test]
fn an_unreadable_config_directory_is_refused_by_path_and_cause() {
    // ROOT READS THROUGH ANY MODE, so this trick cannot produce the
    // permission error the precheck is being asked to name.
    if unsafe { libc::geteuid() } == 0 {
        eprintln!("skipped: running as root, which bypasses directory permissions");
        return;
    }
    // BOTH ARGUMENT SHAPES: the arm this pins refuses FAIL-CLOSED either
    // way, so `--force` must not buy past a directory the walk cannot even
    // stat. With only the bare shape here, an arm that refused when
    // `!force` and fell through otherwise still passed.
    for arguments in [vec!["setup"], vec!["setup", "--force"]] {
        let sandbox = Sandbox::without_config("setup-unreadable-config-dir");
        let config_dir = sandbox.root.join(".config/pns");
        std::fs::create_dir_all(&config_dir).expect("the config directory");
        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o000))
            .expect("lock the directory down");

        let output = sandbox
            .bare()
            .args(&arguments)
            .stdin(std::process::Stdio::null())
            .output()
            .expect("the wizard runs");
        // RESTORED BEFORE ANY ASSERTION CAN PANIC PAST IT: the sandbox's own
        // Drop has to walk this directory to remove it.
        std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700))
            .expect("restore the directory so the sandbox can be cleaned up");

        assert_eq!(output.status.code(), Some(2), "arguments: {arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("could not be checked"),
            "an unreadable directory was not refused by its own cause \
             ({arguments:?}): {stderr}"
        );
        assert!(
            stderr.contains(&config_dir.join("config.toml").display().to_string()),
            "the refusal does not name the config path ({arguments:?}): {stderr}"
        );
        // THE PRE-CHECK RAN FIRST, AND IT REFUSED RATHER THAN REPORTED:
        // reaching the tty check means this arm either printed and carried
        // on, or never fired at all.
        assert!(
            !stderr.contains("not a terminal"),
            "the pre-check did not refuse before the tty check \
             ({arguments:?}): {stderr}"
        );
    }
}

#[test]
fn an_empty_home_is_refused_by_name_before_anything_is_written() {
    // BOTH SHAPES A LAUNCHD-LESS, MISCONFIGURED SHELL CAN HAND A PROCESS:
    // set-but-empty and absent are different environments, and a build that
    // catches only one (the empty check with `unwrap_or_default` in place of
    // `.ok()`, say) still passed with only one case here.
    for home_is_absent in [false, true] {
        let sandbox = Sandbox::without_config(if home_is_absent {
            "setup-home-absent"
        } else {
            "setup-empty-home"
        });
        let mut command = sandbox.bare();
        if home_is_absent {
            command.env_remove("HOME");
        } else {
            // `bare()` points HOME at the sandbox; this overrides it back to
            // empty.
            command.env("HOME", "");
        }
        let output = command
            // KEEPS A STILL-UNFIXED RUN'S RELATIVE `.config/pns/config.toml`
            // write inside the sandbox rather than wherever this test binary
            // happens to run from.
            .current_dir(&sandbox.root)
            .args(["setup"])
            .stdin(std::process::Stdio::null())
            .output()
            .expect("the wizard runs");

        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        // THE EXACT SENTENCE, not just the word: with `HOME` alone, a build
        // that went back to saying "HOME is empty" for BOTH shapes stayed
        // green, and an absent HOME was then reported as an empty one.
        assert!(
            stderr.contains("HOME is unset or empty"),
            "the refusal does not name HOME as unset or empty: {stderr}"
        );
        assert!(
            !sandbox.root.join(".config").exists(),
            "something was written under {} HOME: {stderr}",
            if home_is_absent {
                "an absent"
            } else {
                "an empty"
            }
        );
    }
}
