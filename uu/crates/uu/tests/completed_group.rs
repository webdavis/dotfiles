mod support;

use std::fs::{self, File};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use support::Home;

fn until(mut ready: impl FnMut() -> bool) {
    let start = Instant::now();
    while !ready() {
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "fixture timed out"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal zero only observes the private fixture; it sends no signal.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn unlocked(home: &std::path::Path) -> bool {
    let lock = File::open(home.join(".local/state/uu/run.lock")).unwrap();
    // SAFETY: the descriptor belongs to this fixture; closing it releases the lock.
    unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) == 0 }
}

// These roles run only inside the private updater's two command lanes.
#[test]
#[ignore]
fn lane_fixture() {
    let home = std::path::PathBuf::from(std::env::var_os("HOME").unwrap());
    match std::env::var("UU_GROUP_ROLE").unwrap().as_str() {
        "leader" => {
            // This fixture must exit before its worker, reproducing the lost group.
            #[allow(clippy::zombie_processes)]
            let worker = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "lane_fixture", "--ignored"])
                .env("UU_GROUP_ROLE", "worker")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            fs::write(home.join("worker-pid"), worker.id().to_string()).unwrap();
            fs::write(home.join("leader-pid"), std::process::id().to_string()).unwrap();
            until(|| home.join("worker-ready").exists());
        }
        "worker" => {
            // SAFETY: only this fixture ignores TERM, so KILL is required for cleanup.
            unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) };
            // SAFETY: getpgrp observes only this process's original group.
            fs::write(
                home.join("worker-group"),
                unsafe { libc::getpgrp() }.to_string(),
            )
            .unwrap();
            assert!(!unlocked(&home));
            fs::write(home.join("worker-ready"), "yes").unwrap();
            until(|| {
                if unlocked(&home) {
                    fs::write(home.join("worker-after-unlock"), "yes").unwrap();
                    return true;
                }
                home.join("release").exists()
            });
        }
        "active" => {
            fs::write(home.join("lane-two-ready"), "yes").unwrap();
            until(|| home.join("release").exists());
        }
        role => panic!("unexpected role: {role}"),
    }
}

fn interrupt_after_completed_lane(signal: i32, name: &str) {
    let home = Home::new(name);
    let fixture = std::env::current_exe().unwrap();
    let args = |role| {
        format!(
            "[\"/usr/bin/env\", \"UU_GROUP_ROLE={role}\", {fixture:?}, \
             \"--exact\", \"lane_fixture\", \"--ignored\"]"
        )
    };
    let alert = home.write_stub("alert", "touch \"$HOME/alert-ran\"\n");
    let home = home.with_config(&format!(
        "[lanes.first]\ntype = \"command\"\nrun = {}\n\
         [lanes.second]\ntype = \"command\"\nrun = {}\n\
         [alerts]\nbinary = {alert:?}\n",
        args("leader"),
        args("active")
    ));
    let mut run = Command::new(env!("CARGO_BIN_EXE_uu"))
        .arg("run")
        .env("HOME", &home.dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    until(|| home.dir.join("lane-two-ready").exists());
    let worker = fs::read_to_string(home.dir.join("worker-pid"))
        .unwrap()
        .parse()
        .unwrap();
    let worker_survived_lane = alive(worker);
    // SAFETY: this is our unreaped private updater child, never a live run.
    assert_eq!(unsafe { libc::kill(run.id() as i32, signal) }, 0);
    until(|| run.try_wait().unwrap().is_some());
    until(|| !alive(worker) || home.dir.join("worker-after-unlock").exists());
    // Release a surviving regression fixture before asserting the result.
    fs::write(home.dir.join("release"), "yes").unwrap();
    until(|| !alive(worker));

    assert_eq!(run.wait().unwrap().code(), Some(128 + signal));
    let leader = fs::read_to_string(home.dir.join("leader-pid")).unwrap();
    assert_eq!(
        leader,
        fs::read_to_string(home.dir.join("worker-group")).unwrap()
    );
    assert_ne!(leader, run.id().to_string(), "producer joined uu's group");
    assert!(unlocked(&home.dir), "completed run retained its lock");
    let log = fs::read_to_string(home.dir.join(".local/log/uu/uu.log")).unwrap();
    assert_eq!(log.matches("uu: run interrupted").count(), 1, "{log}");
    assert!(!home.marker().exists());
    assert!(!home.dir.join("alert-ran").exists());
    assert!(
        !home.dir.join("worker-after-unlock").exists(),
        "completed lane's worker wrote after the updater released its lock"
    );
    assert!(
        !worker_survived_lane,
        "completed lane's group survived into lane two"
    );
}

#[test]
fn sigint_in_next_lane_cannot_leave_a_completed_lanes_worker_running() {
    interrupt_after_completed_lane(libc::SIGINT, "completed-group-int");
}

#[test]
fn sigterm_in_next_lane_cannot_leave_a_completed_lanes_worker_running() {
    interrupt_after_completed_lane(libc::SIGTERM, "completed-group-term");
}
