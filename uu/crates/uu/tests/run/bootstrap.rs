use super::*;

#[test]
fn bootstrap_of_a_lane_whose_type_has_no_bootstrap_step_is_refused_naming_the_type() {
    let home = Home::new("bootstrap-unsupported").with_herdr_lane(0);
    let output = home.uu(&["bootstrap", "herdr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("type `herdr` has no bootstrap step"),
        "{output:?}"
    );
}

#[test]
fn bootstrap_of_an_undeclared_lane_is_refused_like_a_run_of_one() {
    let home = Home::new("bootstrap-undeclared").with_herdr_lane(0);
    let output = home.uu(&["bootstrap", "hedr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        stderr(&output).contains("no `[lanes.hedr]` block"),
        "{output:?}"
    );
}

#[test]
fn bootstrap_of_a_configless_named_lane_is_refused_without_creating_history() {
    let home = Home::new("bootstrap-no-config");
    let output = home.uu(&["bootstrap", "herdr"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stderr(&output).contains("no config"), "{output:?}");
    assert!(!home.dir.join(".local/state/uu").exists());
}

#[test]
fn usage_lists_bootstrap_beside_run_doctor_and_schedule() {
    let home = Home::new("bootstrap-usage");
    let output = home.uu(&[]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(
        stderr(&output).contains("uu bootstrap <lane>"),
        "{output:?}"
    );
}

#[test]
fn bootstrap_posts_no_record_and_leaves_the_marker_and_streaks_alone() {
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };
    let home = Home::new("bootstrap-isolated");
    let inventory = home.dir.join("inventory.json");
    fs::write(
        &inventory,
        r#"{"plugins":{"alpha":[{"scope":"user","version":"2"}]}}"#,
    )
    .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let done = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let finish = done.clone();
    let received = calls.clone();
    let server = std::thread::spawn(move || {
        let until = Instant::now() + Duration::from_millis(450);
        while !finish.load(Ordering::Relaxed) && Instant::now() < until {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    received.fetch_add(1, Ordering::Relaxed);
                    stream
                        .set_read_timeout(Some(Duration::from_millis(40)))
                        .unwrap();
                    let mut data = [0; 4096];
                    let _ = stream.read(&mut data);
                    let _ = stream.write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    );
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(error) => panic!("{error}"),
            }
        }
    });
    let pns = home.write_stub("pns", "printf called >\"$HOME/alert-called\"\n");
    let home=home.with_config(&format!("[lanes.claude-plugins]\ninventory = {inventory:?}\n[records]\nurl = \"http://{address}/record\"\nkey = \"owned-test-key\"\n[alerts]\nbinary = {pns:?}\n"));
    let state = home.dir.join(".local/state/uu");
    let lane = state.join("lanes/claude-plugins");
    fs::create_dir_all(&lane).unwrap();
    fs::write(home.marker(), "123\n").unwrap();
    fs::write(lane.join("streak"), "2\n").unwrap();
    fs::write(lane.join("pending"), "2\n").unwrap();
    let output = home.uu(&["bootstrap", "claude-plugins"]);
    done.store(true, Ordering::Relaxed);
    server.join().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(stdout(&output).contains("baseline"), "{output:?}");
    assert_eq!(
        fs::read_to_string(lane.join("snapshot.tsv")).unwrap(),
        "alpha\t2\n"
    );
    assert_eq!(
        calls.load(Ordering::Relaxed),
        0,
        "bootstrap posted a weekly record"
    );
    assert_eq!(fs::read_to_string(home.marker()).unwrap(), "123\n");
    assert_eq!(fs::read_to_string(lane.join("streak")).unwrap(), "2\n");
    assert_eq!(fs::read_to_string(lane.join("pending")).unwrap(), "2\n");
    assert!(!home.dir.join("alert-called").exists());
}

#[test]
fn a_failed_plugin_bootstrap_prints_its_report_and_exits_one() {
    let home = Home::new("bootstrap-failed");
    let inventory = home.dir.join("missing.json");
    let home = home.with_config(&format!(
        "[lanes.claude-plugins]\ninventory = {inventory:?}\n"
    ));
    let output = home.uu(&["bootstrap", "claude-plugins"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stdout(&output).contains("NOT COMPARED"), "{output:?}");
    assert!(!home.marker().exists());
}
