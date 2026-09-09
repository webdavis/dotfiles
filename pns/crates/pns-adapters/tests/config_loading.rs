use pns_adapters::{ConfigError, LoadOutcome, load_config};
use std::os::unix::fs::{FileTypeExt, symlink};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn a_fifo_at_the_config_path_is_refused_without_waiting_for_a_writer() {
    const CHILD_PATH: &str = "PNS_CONFIG_FIFO_TEST_PATH";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        assert!(matches!(
            load_config(std::path::Path::new(&path)),
            Err(ConfigError::Unreadable(message)) if !message.is_empty()
        ));
        return;
    }
    let path = std::env::temp_dir().join(format!("pns-config-fifo-{}", std::process::id()));
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&path)
            .status()
            .expect("create the private pipe")
            .success()
    );
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "a_fifo_at_the_config_path_is_refused_without_waiting_for_a_writer",
            "--nocapture",
        ])
        .env(CHILD_PATH, &path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run the loader in a bounded child");
    let deadline = Instant::now() + Duration::from_millis(400);
    let completed = loop {
        if child.try_wait().expect("observe the loader").is_some() {
            break true;
        }
        if Instant::now() >= deadline {
            child.kill().expect("stop the blocked loader");
            break false;
        }
        std::thread::sleep(Duration::from_millis(2));
    };
    let output = child.wait_with_output().expect("reap the loader");
    assert!(path.symlink_metadata().unwrap().file_type().is_fifo());
    assert!(completed, "the loader waited for a FIFO writer");
    assert!(
        output.status.success(),
        "the loader did not refuse the FIFO: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn a_regular_config_and_its_symlink_still_load_the_selected_plugin() {
    let dir = std::env::temp_dir().join(format!("pns-config-link-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("config.toml");
    std::fs::write(&path, "[plugins.hue]\nenabled = true\n").unwrap();
    let link = dir.join("linked.toml");
    symlink(&path, &link).unwrap();
    for input in [&path, &link] {
        match load_config(input) {
            Ok(LoadOutcome::Loaded(config)) => assert!(config.plugins["hue"].enabled),
            other => panic!("expected the configured plugin, got {other:?}"),
        }
    }
}
