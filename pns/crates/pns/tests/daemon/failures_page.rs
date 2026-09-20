//! The failure page's lifetime, which is its daemon's.
//!
//! THE PAGE IS THE ONE LONG-LIVED CHILD. It is detached and in a group of its
//! own, so a daemon that stopped left it running under pid 1 holding the fixed
//! port, and every daemon started after that spawned a page that could never
//! bind. Both halves of the tie are pinned here: the child that leaves when its
//! parent does, and the daemon that stops the child before it exits.

use super::lifecycle::process_lives;
use super::*;

/// `ONE_CHANNEL` with the page ON, pointed at a port this test owns. A table
/// written twice is a config TOML refuses, so the shipped `[failures]` block
/// is replaced rather than appended to.
fn page_config(port: u16) -> String {
    ONE_CHANNEL.replace(
        "page_enabled = false\n",
        &format!("page_enabled = true\npage_port = {port}\n"),
    )
}

/// A port nothing is listening on, taken and released so two tests running
/// beside each other cannot serve on the same number.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("a loopback port")
        .local_addr()
        .expect("its address")
        .port()
}

/// THE CHILD'S OWN HALF: a page whose parent is gone puts itself down, with
/// nothing left running to notice it.
#[test]
fn the_page_exits_once_the_process_that_started_it_is_gone() {
    let sandbox = Sandbox::new("daemon-page-orphaned");
    sandbox.write_config(&page_config(free_port()));
    // A STAND-IN FOR THE DAEMON: it starts the page detached, records both
    // pids and then waits, so the test decides when the parent dies rather
    // than racing it.
    let mut parent = Command::new("/bin/bash")
        .env("HOME", &sandbox.root)
        .arg("-c")
        .arg(format!(
            "{engine} failures serve >\"{root}/page.out\" 2>&1 &\n\
             printf '%s %s' \"$$\" \"$!\" >\"{root}/page.pids\"\n\
             sleep 60",
            engine = support::ENGINE,
            root = sandbox.display()
        ))
        .spawn()
        .expect("the stand-in parent starts");

    let page = poll_until(|| {
        let record = std::fs::read_to_string(sandbox.path("page.pids")).ok()?;
        let pids: Vec<String> = record.split_whitespace().map(str::to_string).collect();
        let [_, page] = pids.as_slice() else {
            return None;
        };
        process_lives(page).then(|| page.clone())
    })
    .expect("the page never started");

    let _ = parent.kill();
    let _ = parent.wait();

    assert!(
        poll_until(|| (!process_lives(&page)).then_some(())).is_some(),
        "the page ({page}) outlived the process that started it; it said: {}",
        std::fs::read_to_string(sandbox.path("page.out")).unwrap_or_default()
    );
}

/// THE DAEMON'S OWN HALF: the stop signal launchd sends takes the page with
/// it, so the port is free for the daemon that replaces it.
#[test]
fn a_stopped_daemon_takes_its_failure_page_with_it() {
    let sandbox = Sandbox::new("daemon-page-stopped-with-daemon");
    sandbox.allow_slow("waits out a real daemon start, a page spawn and a signalled stop");
    sandbox.write_config(&page_config(free_port()));
    let mut guard = DaemonGuard::start(&sandbox, TICK_MS);
    let page = poll_until(|| page_child_of(guard.pid())).unwrap_or_else(|| {
        panic!("the daemon never started its page: {}", guard.said());
    });

    // SAFETY: `kill` reads and writes no memory here, and the pid is the
    // guard's own live, unreaped child.
    unsafe { libc::kill(guard.pid(), libc::SIGTERM) };
    assert!(
        guard.exited_within(Duration::from_secs(10)).is_some(),
        "the daemon ignored its stop signal: {}",
        guard.said()
    );
    assert!(
        poll_until(|| (!process_lives(&page.to_string())).then_some(())).is_some(),
        "the page ({page}) outlived the daemon that started it"
    );
}

/// The `pns failures serve` among a daemon's children, by the argv it was
/// spawned with.
fn page_child_of(daemon: i32) -> Option<i32> {
    let listed = Command::new("/usr/bin/pgrep")
        .args(["-P", &daemon.to_string()])
        .output()
        .ok()?;
    String::from_utf8_lossy(&listed.stdout)
        .split_whitespace()
        .filter_map(|pid| pid.parse::<i32>().ok())
        .find(|pid| argv_of(*pid).contains("failures serve"))
}

fn argv_of(pid: i32) -> String {
    Command::new("/bin/ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .map(|listed| String::from_utf8_lossy(&listed.stdout).into_owned())
        .unwrap_or_default()
}

/// THE PAGE IS NOT A DELIVERY. Bounded like one it was killed and respawned
/// every `CHILD_TICKS`, twice a minute at the production clock, and each new
/// child wrote the bind line again: sixteen thousand copies of it in the log
/// the operator reads.
#[test]
fn the_page_is_the_same_process_tick_after_tick() {
    let sandbox = Sandbox::new("daemon-page-not-respawned");
    sandbox.allow_slow("waits out several times the bound that used to kill the page");
    sandbox.write_config(&page_config(free_port()));
    let guard = DaemonGuard::start(&sandbox, FAST_TICK_MS);
    let first = poll_until(|| page_child_of(guard.pid())).unwrap_or_else(|| {
        panic!("the daemon never started its page: {}", guard.said());
    });
    // WELL PAST `CHILD_TICKS` (30) AT THIS TICK, which is where the bound used
    // to fall: 600 ms is twice the 300 ms it took to kill the listener.
    std::thread::sleep(Duration::from_millis(600));
    assert_eq!(
        page_child_of(guard.pid()),
        Some(first),
        "the page was replaced rather than left serving: {}",
        guard.said()
    );
}
