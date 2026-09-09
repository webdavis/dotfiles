use super::*;
use std::io::Write;

#[test]
fn producer_death_stops_descendants_before_the_original_deadline() {
    const FIXTURE: &str = "PNS_OWNED_GROUP_FIXTURE";
    if let Some(ready) = std::env::var_os(FIXTURE) {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "sleep 10 & printf '%s %s\\n' \"$$\" \"$!\" > \"$1\"; wait",
            "sh",
        ]);
        command.arg(ready);
        super::super::super::bounded::run_bounded(command, None, Duration::from_millis(600), 0);
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "pns-group-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let ready = root.join("ready");
    let test_name = concat!(
        module_path!(),
        "::producer_death_stops_descendants_before_the_original_deadline"
    );
    let test_name = test_name.split_once("::").unwrap().1;
    let mut producer = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", test_name, "--nocapture"])
        .env(FIXTURE, &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .unwrap();
    let expires = Instant::now() + Duration::from_millis(300);
    let text = loop {
        if let Ok(text) = std::fs::read_to_string(&ready)
            && text.ends_with('\n')
        {
            break text;
        }
        if Instant::now() >= expires {
            let _ = producer.kill();
            let _ = producer.wait();
            panic!("owned descendant fixture did not become ready");
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    let pids: Vec<libc::pid_t> = text
        .split_whitespace()
        .map(|word| word.parse().unwrap())
        .collect();
    assert_eq!(pids.len(), 2);
    // SAFETY: these pids were written by the private child we just launched.
    let group = unsafe { libc::getpgid(pids[0]) };
    assert!(group > 1 && group != unsafe { libc::getpgrp() });
    producer.kill().unwrap();
    producer.wait().unwrap();
    let expires = Instant::now() + Duration::from_millis(150);
    let absent = loop {
        // SAFETY: signal zero only observes these owned fixture processes.
        let absent = pids.iter().all(|pid| unsafe { libc::kill(*pid, 0) } == -1);
        if absent || Instant::now() >= expires {
            break absent;
        }
        std::thread::sleep(Duration::from_millis(1));
    };
    if !absent {
        // The fault path stops only the group reported by our owned fixture.
        // Neither the test runner nor unrelated processes are in that group.
        unsafe {
            libc::kill(-group, libc::SIGKILL);
        }
    }
    let cleanup_expires = Instant::now() + Duration::from_millis(150);
    while pids.iter().any(|pid| unsafe { libc::kill(*pid, 0) } == 0) {
        assert!(
            Instant::now() < cleanup_expires,
            "owned fixture cleanup did not finish"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    std::fs::File::create(root.join("observation"))
        .unwrap()
        .write_all(format!("pids={pids:?}, group={group}, absent={absent}\n").as_bytes())
        .unwrap();
    assert!(
        absent,
        "producer death must release the group before its 600ms deadline"
    );
}
