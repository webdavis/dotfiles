use super::*;
use std::fs::{self, DirBuilder, File};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

fn files() -> (PathBuf, File, File) {
    let root = std::env::temp_dir().join(format!(
        "uu-file-output-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    DirBuilder::new().mode(0o700).create(&root).unwrap();
    fs::write(root.join("input"), [0, 255, 128, 10]).unwrap();
    let input = File::open(root.join("input")).unwrap();
    let output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(root.join("output"))
        .unwrap();
    (root, input, output)
}
fn bounded(millis: u64) -> SystemRunner {
    SystemRunner::for_lane(
        "file-output",
        Duration::from_millis(millis),
        Duration::from_millis(millis),
    )
}
#[test]
fn file_output_preserves_binary_stdin_and_stdout_without_a_text_conversion_or_tail_cap() {
    let (root, input, output) = files();
    let original: Vec<u8> = (0..200_000).map(|i| (i % 256) as u8).collect();
    fs::write(root.join("input"), &original).unwrap();
    bounded(400)
        .run_to_file("/bin/cat", &[], input, output)
        .unwrap();
    assert_eq!(fs::read(root.join("output")).unwrap(), original);
}
#[test]
fn file_output_reports_the_childs_failure_and_stderr() {
    let (_root, input, output) = files();
    let error = bounded(400)
        .run_to_file(
            "/bin/sh",
            &["-c", "printf 'compress failed' >&2; exit 7"],
            input,
            output,
        )
        .unwrap_err();
    assert!(
        error.contains("exit 7") && error.contains("compress failed"),
        "{error}"
    );
}
#[test]
fn file_output_never_spawns_after_its_lane_budget_is_spent() {
    let (root, input, output) = files();
    let marker = root.join("spawned");
    let result = bounded(0).run_to_file(
        "/bin/sh",
        &[
            "-c",
            "printf spawned > \"$1\"",
            "owned",
            marker.to_str().unwrap(),
        ],
        input,
        output,
    );
    assert!(result.is_err());
    assert!(!marker.exists());
}
#[test]
fn file_output_kills_its_child_at_the_existing_lane_deadline() {
    let (root, input, output) = files();
    let pid_path = root.join("pid");
    let started = Instant::now();
    let error = bounded(100)
        .run_to_file(
            "/bin/sh",
            &[
                "-c",
                "printf '%s' \"$$\" > \"$1\"; exec /bin/sleep 30",
                "owned",
                pid_path.to_str().unwrap(),
            ],
            input,
            output,
        )
        .unwrap_err();
    assert!(started.elapsed() < Duration::from_millis(800), "{error}");
    let pid: i32 = fs::read_to_string(&pid_path)
        .expect("child started")
        .parse()
        .unwrap();
    // This PID belongs to the child whose birth was captured above; no signal is sent.
    assert_eq!(
        unsafe { libc::kill(pid, 0) },
        -1,
        "owned compressor survived"
    );
}

#[test]
fn file_output_refuses_a_write_error_instead_of_reporting_a_complete_archive() {
    let (root, input, output) = files();
    drop(output);
    let readonly = File::open(root.join("output")).unwrap();
    let error = bounded(400)
        .run_to_file("/bin/cat", &[], input, readonly)
        .unwrap_err();
    assert!(error.contains("write"), "{error}");
}
#[test]
fn file_output_waits_for_descendants_holding_only_stdout_and_stops_the_owned_group() {
    let (root, input, output) = files();
    let pid = root.join("child");
    let group = root.join("group");
    let started = Instant::now();
    let result = bounded(100).run_to_file("/bin/sh", &["-c", "printf '%s' \"$$\" > \"$1\"; exec 2>/dev/null; /bin/sleep 30 & printf '%s' \"$!\" > \"$2\"; exit 0", "owned", group.to_str().unwrap(), pid.to_str().unwrap()], input, output);
    let child: i32 = fs::read_to_string(pid)
        .expect("owned child started")
        .parse()
        .unwrap();
    let owner: i32 = fs::read_to_string(group).unwrap().parse().unwrap();
    let alive = unsafe { libc::kill(child, 0) } == 0;
    // Even a broken output-EOF guard leaves no running fixture group behind.
    if alive && unsafe { libc::getpgid(child) } == owner {
        unsafe { libc::kill(-owner, libc::SIGKILL) };
    }
    assert!(
        result.is_err(),
        "the descendant kept stdout open past the lane deadline"
    );
    assert!(started.elapsed() < Duration::from_millis(800));
    assert!(!alive, "owned stdout holder survived the watchdog");
}
