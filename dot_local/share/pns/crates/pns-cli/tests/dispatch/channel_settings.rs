use super::*;

#[test]
fn a_watch_card_toggle_of_the_wrong_type_is_refused_out_loud() {
    // The config layer refuses a non-boolean `enabled` by name, and a plugin
    // SETTING that quietly reads false is the same defect one level down: an
    // operator who wrote true in quotes got no card and no reason for it.
    let sandbox = Sandbox::new("watch-card-wrong-type");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "[plugins.mobile]\nenabled = true\ntype = \"moshi\"\nmobile_watch_card = \"true\"\n\
         [plugins.hermes]\nenabled = true\n",
    )
    .expect("config");
    let mut command = sandbox.pns();
    command.env("PNS_PHONE_INPUT_AGE", "0");
    sandbox.stub_herdr(&mut command, true);
    let output = run(command
        .args(["--agent", "claude", "--state", "done", "--detail", "x"])
        .args(["--pane", "t1:p2", "--long-running"]));
    assert!(
        stderr(&output).contains("mobile_watch_card"),
        "the refusal names the setting: {output:?}"
    );
    assert!(
        !sandbox.fired("mobile"),
        "and the card stays off, which is the default it fell back to"
    );
}

#[test]
fn one_typod_table_name_costs_a_configured_machine_no_channel() {
    // THE FILE PARSED, which is the whole distinction. Every credential in it
    // is in hand and the composition root has already read them off it, so a
    // single-character slip in one table NAME is loud and nothing more. The
    // core fallback beside it is for a file nobody could read; applying it
    // here silently stopped the durable paper trail this crate elsewhere calls
    // never-suppressible, and took the lights with it.
    let sandbox = Sandbox::new("typod-table-name");
    sandbox.write_config(
        "[plugins.hermess]\nenabled = true\n\
         [plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
         [plugins.hermes]\nenabled = true\n[plugins.macos-banner]\nenabled = true\n",
    );
    let output = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done"])
        .args(["--project", "dotfiles", "--detail", "a summary"]));

    assert!(sandbox.fired("mobile"), "stderr: {}", stderr(&output));
    assert!(
        sandbox.fired("hermes"),
        "the durable route survives a typo in an unrelated table: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("unknown plugin `hermess`"),
        "and it is still LOUD: {}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("running every built-in plugin"),
        "the line says what still runs: {}",
        stderr(&output)
    );
}

#[test]
fn a_broken_config_says_so_in_pulse_mode_too_instead_of_dying_quietly() {
    // Event mode has always printed the sanitized warning for a config it
    // could not read. Pulse mode collapsed unreadable and malformed together
    // with absent into one silent no-op, so the operator's only signal that a
    // config was broken was lights that stopped working.
    let sandbox = support::Sandbox::new("pulse-broken-config");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "this is not toml\n",
    )
    .expect("config");
    let output = sandbox
        .bare()
        .args(["pulse", "1"])
        .output()
        .expect("the engine runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("pns: config error"),
        "a broken config is loud in pulse mode: {stderr}"
    );
    assert!(output.status.success(), "and still exits zero");
}

#[test]
fn an_absent_config_stays_silent_in_pulse_mode() {
    // The other half of the rule: absent is not broken. A machine that never
    // opted into a config must not be nagged on every long command.
    let sandbox = support::Sandbox::without_config("pulse-absent-config");
    let output = sandbox
        .bare()
        .args(["pulse", "0"])
        .output()
        .expect("the engine runs");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(output.status.success());
}

#[test]
fn pulse_help_prints_its_own_usage_before_any_config_load() {
    // H-C: `pulse --help` used to load the config first, so with none it
    // silently exited 0 and with one it pulsed the room red, because
    // `exit_behaviour` read the word as a failing, non-numeric code. Reading
    // the word first means help answers with no machine read at all, even
    // with no config on disk.
    let sandbox = support::Sandbox::without_config("pulse-help");
    // H-B: help wins in flag position anywhere in the tail, not only right
    // after the subcommand, so `pulse 0 --help` and `pulse --help 0` are
    // slices here rather than the single-word cases above. `0 -h` pins the
    // short flag late too: a mutant that recognizes a late `--help` but not
    // a late `-h` would still pass the `0 --help` case above.
    for args in [
        ["--help"].as_slice(),
        ["-h"].as_slice(),
        ["0", "--help"].as_slice(),
        ["--help", "0"].as_slice(),
        ["0", "-h"].as_slice(),
    ] {
        let output = sandbox
            .bare()
            .arg("pulse")
            .args(args)
            .output()
            .expect("the engine runs");
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        // The phrase names PULSE_USAGE specifically, not the global USAGE
        // text, which also mentions "pns pulse" in its subcommand list.
        assert!(
            stdout(&output).contains("success pulse"),
            "{args:?}: {output:?}"
        );
        assert_eq!(stderr(&output), "", "{args:?}: {output:?}");
    }
}

#[test]
fn pulse_refuses_a_code_it_cannot_read_instead_of_guessing_it_failed() {
    // H-C: `exit_behaviour` now answers `None` for anything that is not an
    // ASCII-digit run (or empty), and `pulse_mode` reads that as a refusal
    // rather than a failure pulse. `pulse oops`, `-0` and padded zeroes used
    // to flash the room red on a code nobody proved.
    let sandbox = support::Sandbox::without_config("pulse-refuses");
    // H-B: an unknown word is refused wherever it lands, so `0 stray` (a
    // second tail token that is not help) is a slice case alongside the
    // single-word ones above. `0 a b` pins a THIRD tail token: a
    // `tail.len() == 2` mutant still refuses two-token tails and would only
    // be caught by a tail longer than that.
    for args in [
        ["oops"].as_slice(),
        ["-0"].as_slice(),
        [" 0"].as_slice(),
        ["0\n"].as_slice(),
        ["0", "stray"].as_slice(),
        ["0", "a", "b"].as_slice(),
    ] {
        let output = sandbox
            .bare()
            .arg("pulse")
            .args(args)
            .output()
            .expect("the engine runs");
        assert_eq!(output.status.code(), Some(2), "{args:?}: {output:?}");
        // The phrase names PULSE_USAGE specifically, not the global USAGE
        // text, which also mentions "pns pulse" in its subcommand list.
        assert!(
            stderr(&output).contains("success pulse"),
            "{args:?}: {output:?}"
        );
        assert_eq!(stdout(&output), "", "{args:?}: {output:?}");
    }
}

#[test]
fn an_unknown_plugin_never_resurrects_a_disabled_pulse() {
    // The full-roster fallback is an EVENT-mode rule: it keeps notifications
    // working when a config is wrong. Applying it to the pulse turns a
    // deliberate `enabled = false` back on over an unrelated typo elsewhere.
    //
    // A pulse is silent either way, so the bridge address IS the observation:
    // a listener nobody should ever reach.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    listener.set_nonblocking(true).expect("nonblocking");

    let sandbox = support::Sandbox::new("pulse-disabled-plus-typo");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        format!(
            "[plugins.hue]\nenabled = false\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
             [plugins.typo]\nenabled = true\n"
        ),
    )
    .expect("config");
    let child = sandbox
        .bare()
        .args(["pulse", "1"])
        .spawn()
        .expect("the engine starts");
    assert_eq!(
        wait_bounded(child, std::time::Duration::from_secs(2)),
        Some(0),
        "a disabled pulse exits at once rather than talking to a bridge"
    );
    assert!(
        matches!(listener.accept(), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "a disabled pulse must not reach the bridge, whatever else the config got wrong"
    );
}

#[test]
fn the_pulse_config_warning_says_what_pulse_mode_actually_did() {
    // The full line, not its prefix: the old suffix promised every built-in
    // plugin would run, which is an event-mode sentence and false here.
    let sandbox = support::Sandbox::new("pulse-warning-wording");
    std::fs::create_dir_all(sandbox.path(".config/pns")).expect("config dir");
    std::fs::write(
        sandbox.path(".config/pns/config.toml"),
        "this is not toml\n",
    )
    .expect("config");
    let output = sandbox
        .bare()
        .args(["pulse", "1"])
        .output()
        .expect("the engine runs");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr).trim_end(),
        "pns: config error (key with no value, expected `=` at line 1); no pulse",
        "the pulse warning names the pulse outcome"
    );
    assert!(output.status.success());
}
