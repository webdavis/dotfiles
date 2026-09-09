use std::process::Command;

#[test]
fn help_exits_zero_without_settings() {
    let result = Command::new(env!("CARGO_BIN_EXE_lights"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("Usage: lights"));
    assert!(result.stderr.is_empty());
}

#[test]
fn unknown_command_exits_one_on_stderr() {
    refusal("bogus");
}

#[test]
fn unknown_flag_exits_one_on_stderr() {
    refusal("--bogus");
}

fn refusal(arg: &str) {
    let result = Command::new(env!("CARGO_BIN_EXE_lights"))
        .arg(arg)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).starts_with("lights: "));
}

#[test]
fn config_failures_reach_process_exit_five() {
    let root = std::env::temp_dir().join(format!("lights-process-{}", std::process::id()));
    std::fs::create_dir_all(root.join("lights")).unwrap();
    for content in [
        None,
        Some("malformed = ["),
        Some("[controller]\ntype='hue'\naddress='192.0.2.1'"),
    ] {
        if let Some(content) = content {
            std::fs::write(root.join("lights/config.toml"), content).unwrap();
        }
        let output = Command::new(env!("CARGO_BIN_EXE_lights"))
            .env_clear()
            .env("HOME", &root)
            .env("XDG_CONFIG_HOME", &root)
            .env("XDG_DATA_HOME", &root)
            .env("XDG_STATE_HOME", &root)
            .env("XDG_CACHE_HOME", &root)
            .env("XDG_RUNTIME_DIR", &root)
            .env("TMPDIR", &root)
            .env("TMP", &root)
            .env("TEMP", &root)
            .env("CLAUDE_CONFIG_DIR", &root)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .arg("toggle")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(5));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("lights: "));
    }
}
