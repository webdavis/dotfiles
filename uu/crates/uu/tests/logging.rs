mod support;

use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;
use std::process::{Command, Output};
use support::{Home, stdout};

// Read only fields from uu's generated fixture, with an ASCII scratch HOME.
fn stream_path(plist: &str, key: &str) -> PathBuf {
    let (_, field) = plist.split_once(&format!("<key>{key}</key>")).unwrap();
    let (_, value) = field.split_once("<string>").unwrap();
    value.split_once("</string>").unwrap().0.into()
}

fn output_file(path: &PathBuf) -> File {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap()
}

fn scheduled(home: &Home) -> (Output, PathBuf) {
    let rendered = home.uu(&["schedule", "render"]);
    assert!(rendered.status.success());
    let plist = stdout(&rendered);
    let stderr = stream_path(&plist, "StandardErrorPath");
    let output = Command::new(env!("CARGO_BIN_EXE_uu"))
        .arg("run")
        .env("HOME", &home.dir)
        .stdout(output_file(&stream_path(&plist, "StandardOutPath")))
        .stderr(output_file(&stderr))
        .output()
        .unwrap();
    (output, stderr)
}

#[test]
fn scheduled_output_records_each_run_once() {
    let home = Home::new("scheduled-log")
        .with_config("[lanes.fixture]\ntype = \"command\"\nrun = [\"/usr/bin/true\"]\n");
    assert!(scheduled(&home).0.status.success());
    assert!(scheduled(&home).0.status.success());
    let log = fs::read_to_string(home.dir.join(".local/log/uu/uu.log")).unwrap();
    assert_eq!(log.matches("done, 0 failure(s)").count(), 2, "{log}");
    assert_eq!(
        log.matches("this run was logged here and nowhere else")
            .count(),
        2,
        "{log}"
    );
}

#[test]
fn scheduled_startup_errors_remain_visible_before_the_run_log_opens() {
    let home = Home::new("scheduled-startup-error").with_config("");
    // Render a valid schedule before making config loading fail.
    let rendered = stdout(&home.uu(&["schedule", "render"]));
    let stderr = stream_path(&rendered, "StandardErrorPath");
    fs::write(home.dir.join(".config/uu/config.toml"), "[invalid").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_uu"))
        .arg("run")
        .env("HOME", &home.dir)
        .stdout(output_file(&stream_path(&rendered, "StandardOutPath")))
        .stderr(output_file(&stderr))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        fs::read_to_string(stderr)
            .unwrap()
            .contains("not valid TOML")
    );
    assert!(!home.dir.join(".local/log/uu/uu.log").exists());
}
