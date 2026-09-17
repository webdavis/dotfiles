use crate::alert::Alerter;
use crate::lanes::CommandRunner;
use std::fs;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// The host libtest process must never install signal handlers or receive signals.
#[test]
#[ignore]
fn subject() {
    super::install_interruption().unwrap();
    let home = std::env::var("UU_TEST_HOME").unwrap();
    let script = format!("{home}/producer");
    let runner = crate::runner::SystemRunner::for_lane(
        "fixture",
        Duration::from_secs(2),
        Duration::from_secs(2),
    );
    let mode = std::env::var("UU_TEST_RUNNER").unwrap();
    let result = match mode.as_str() {
        "plain" => runner.run(&script, &[]).map(|_| ()),
        "environment" => runner
            .run_in(
                &script,
                &[],
                &std::collections::BTreeMap::from([("HOME".into(), home.clone())]),
            )
            .map(|_| ()),
        "file" => runner.run_to_file(
            &script,
            &[],
            fs::File::open("/dev/null").unwrap(),
            fs::File::create(format!("{home}/output")).unwrap(),
        ),
        "step" => runner
            .run_with_deadline(&script, &[], Duration::from_secs(2))
            .map(|_| ()),
        "alert" => crate::delivery::PnsAlerter.alert(&script, &[]),
        _ => panic!("unknown fixture"),
    };
    fs::write(format!("{home}/result"), format!("{result:?}")).unwrap();
    // A cancelled runner must refuse every later entry point before spawning.
    let late = ["-c", "touch \"$HOME/late\""];
    assert!(runner.run("/bin/sh", &late).is_err());
    assert!(
        runner
            .run_in(
                "/bin/sh",
                &late,
                &std::collections::BTreeMap::from([("HOME".into(), home.clone()),])
            )
            .is_err()
    );
    assert!(
        runner
            .run_to_file(
                "/bin/sh",
                &late,
                fs::File::open("/dev/null").unwrap(),
                fs::File::create(format!("{home}/late-output")).unwrap()
            )
            .is_err()
    );
    assert!(
        runner
            .run_with_deadline("/bin/sh", &late, Duration::from_secs(2))
            .is_err()
    );
}

const LIVENESS_BOUND: Duration = Duration::from_secs(15);

fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while !condition() {
        if start.elapsed() >= LIVENESS_BOUND {
            return false;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    true
}

fn cancelled(mode: &str) {
    use std::os::unix::fs::PermissionsExt;
    let home = std::env::temp_dir().join(format!("uu-cancel-{mode}-{}", std::process::id()));
    // A PID IS NOT UNIQUE OVER TIME, and the markers below are graded by
    // existence: a run that died before its cleanup would leave `cleaned` or
    // `late` behind for the next holder of this id to read as its own.
    let _ = fs::remove_dir_all(&home);
    fs::create_dir_all(&home).unwrap();
    let script = home.join("producer");
    fs::write(
        &script,
        "#!/bin/sh\ntrap 'echo yes > \"$HOME/cleaned\"; exit 0' TERM\n\
         echo ready > \"$HOME/ready\"\n\
         while [ ! -f \"$HOME/release\" ]; do /bin/sleep 0.01; done\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "interruption::tests::subject", "--ignored"])
        .env("UU_TEST_HOME", &home)
        .env("HOME", &home)
        .env("UU_TEST_RUNNER", mode)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    assert!(wait_for(|| home.join("ready").exists()));
    // SAFETY: child is the unreaped fixture subprocess owned by this test.
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    // A LIVENESS BOUND, NOT A MEASUREMENT: a runner that ignored the signal
    // holds its child until this test writes `release`, so only a regression
    // waits this long. The 400ms it carried was a wall-clock budget for a
    // handler, a kill and a reap while the operator's other agent lanes
    // compile.
    let stopped = wait_for(|| child.try_wait().unwrap().is_some());
    fs::write(home.join("release"), "yes").unwrap();
    let status = child.wait().unwrap();
    let result = fs::read_to_string(home.join("result")).unwrap();
    let cleaned = home.join("cleaned").exists();
    let late = home.join("late").exists();
    fs::remove_dir_all(home).unwrap();
    assert!(stopped, "{mode} ignored interruption: {result}");
    assert!(status.success(), "{mode}: {status}");
    assert!(cleaned, "{mode} left its producer running");
    assert!(result.contains("interrupted"), "{mode}: {result}");
    assert!(!late, "{mode} started another producer after interruption");
}

#[test]
fn plain_runner_cancels() {
    cancelled("plain");
}
#[test]
fn environment_runner_cancels() {
    cancelled("environment");
}
#[test]
fn file_output_runner_cancels() {
    cancelled("file");
}
#[test]
fn bounded_step_cancels() {
    cancelled("step");
}
#[test]
fn alert_child_cancels() {
    cancelled("alert");
}
