use super::*;

// --- the lights quiet window ------------------------------------------------

/// The pulse's whole visible effect at this boundary is whether it dialled, so
/// a bare loopback listener IS the bridge: nothing here speaks CLIP and
/// nothing has to.
pub(super) fn bridge_spy() -> (std::net::TcpListener, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    let port = listener.local_addr().expect("addr").port();
    listener.set_nonblocking(true).expect("nonblocking");
    (listener, port)
}

/// Whether the pulse dialled the bridge inside `limit`.
///
/// ACCEPTING HANGS UP AT ONCE, which is what keeps a test that expects a dial
/// fast: the engine's TLS handshake fails on the closed socket instead of
/// waiting out the ten-second bridge deadline.
///
/// `Duration::ZERO` IS THE RIGHT CALL, not a bug, everywhere the child has
/// already exited: `run` waits for output and a poll loop waits for the
/// child before asking, so a dial that was going to happen has already
/// happened, and a connection still queued is sitting in the accept queue
/// where one non-blocking accept sees it. Waiting any longer there would only
/// be waiting on a dial that was never coming.
pub(super) fn dialled_within(listener: &std::net::TcpListener, limit: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + limit;
    loop {
        match listener.accept() {
            Ok(_) => return true,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("the bridge spy stopped listening: {error}"),
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// The UTC minute of the day, from the epoch alone. Every test below pins the
/// child to `TZ=UTC`, so this is the minute the engine's own clock reads,
/// with no local-time library on this side of the boundary.
pub(super) fn utc_minute_now() -> u16 {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs();
    u16::try_from((epoch % 86_400) / 60).expect("a minute of the day")
}

/// A window `radius` minutes either side of `centre`, wrapped into the day and
/// spelled the way the config takes it. Hours wide on purpose: the child reads
/// its own clock a moment after this one does, and a window that narrow would
/// be timing the test rather than the gate.
pub(super) fn window_around(centre: u16, radius: u16) -> String {
    let start = (centre + 1440 - radius) % 1440;
    let end = (centre + radius) % 1440;
    format!(
        "{:02}:{:02}-{:02}:{:02}",
        start / 60,
        start % 60,
        end / 60,
        end % 60
    )
}

/// The `[plugins.hue]` config the two halves below share, quiet hours apart.
pub(super) fn hue_config(port: u16, quiet_hours: &str) -> String {
    format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         quiet_hours = \"{quiet_hours}\"\n[plugins.hermes]\nenabled = true\n"
    )
}

/// Asia/Tokyo: nine hours ahead of UTC, and no daylight saving since 1951, so
/// the child's own minute of the day is arithmetic on this side and no window
/// here can straddle a transition.
pub(super) const TOKYO_MINUTES_AHEAD: u16 = 9 * 60;

// --- the lamps: which lamp, and what colour ---------------------------------

/// The studio map this repo actually ships, as a config fragment: the room
/// carries the blinks and HCL3 is lifted out for the held states.
///
/// THE ROOM DOES NOT CARRY `blocked`, and that word being here was the whole
/// difference between a fixture and a copy of the shipped file. With it, every
/// lamp in the room answered for the held states, so a lamp-level override that
/// had stopped working entirely still reached the bridge through HCL1 and HCL2
/// and every case below stayed green.
///
/// THE DIM WINDOW IS THE ONE THING IT LEAVES OUT, deliberately: the shipped room
/// states a 22:00-07:00 window, and a wall-clock window would make every case
/// here answer differently depending on the hour the suite happened to run. The
/// window's own behaviour is pinned by the tests that set a clock.
pub(super) const STUDIO_MAP: &str = "[lights]\nrefresh_secs = 20\n\
     [lights.room.\"3F - Studio\"]\nshows = [\"done\", \"failed\"]\n\
     [lights.lamp.\"3F - Studio - HCL3\"]\nshows = [\"loop\", \"blocked\", \"unread\"]\n";

/// One event against a spy bridge: whether the bridge was dialled, and whether
/// the two network legs fired.
///
/// `MOSHI_HOOK_BIN` POINTS NOWHERE, in every case, without exception. The
/// operator's own moshi daemon is a real program on this machine and no test
/// may reach it; the sandbox's channel stubs cover the leg, and this covers the
/// native path that resolves the binary by name.
pub(super) fn lamp_run(
    name: &str,
    hue_extra: &str,
    config: &str,
    args: &[&str],
    mute: Mute,
    presence: Presence,
) -> (bool, bool, bool, bool, Option<i32>) {
    let (listener, port) = bridge_spy();
    let sandbox = Sandbox::new(name);
    // `hue_extra` GOES INSIDE `[plugins.hue]` and the rest comes after every
    // plugin table, because a bare key in a TOML file belongs to whichever
    // table was opened last: appending `quiet_hours` to the end of this put it
    // in `[plugins.hermes]`, where nothing reads it and nothing complains.
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\nbridge = \"127.0.0.1:{port}\"\nkey = \"k\"\n\
         rooms = [\"3F - Studio\"]\n{hue_extra}[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
         [plugins.hermes]\nenabled = true\n{config}"
    ));
    // THE OPERATOR'S OWN MUTE, armed through the subcommand they actually
    // type rather than by writing its state file here: `HOME` is the sandbox,
    // so the expiry lands inside it and no other test can see it.
    let armed = match mute {
        Mute::Nothing => None,
        Mute::Everything => Some(run(sandbox.pns().args(["quiet", "1h"]))),
        Mute::Lights(place) => Some(run(sandbox.pns().args(["lights", "quiet", place, "1h"]))),
    };
    if let Some(armed) = armed {
        assert_eq!(
            armed.status.code(),
            Some(0),
            "the mute is armed before the event: {}",
            stderr(&armed)
        );
    }
    let mut command = sandbox.pns();
    command.env("TZ", "UTC");
    command.env("MOSHI_HOOK_BIN", sandbox.path("no-moshi-hook-here"));
    command.env(
        "PNS_IDLE_SECS",
        match presence {
            Presence::Away => "99999",
            Presence::Desk => "0",
        },
    );
    sandbox.stub_herdr(&mut command, false);
    let mut child = command
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the engine starts");
    // ACCEPTED WHILE THE CHILD IS STILL RUNNING, which is what keeps a dial
    // fast: the spy hangs up the moment it accepts, so the engine's TLS
    // handshake fails at once instead of waiting out the ten-second bridge
    // deadline. AND IT STOPS AT THE CHILD'S EXIT rather than at a fixed
    // deadline, so a case that expects NO dial costs the child's own runtime
    // instead of five seconds of waiting for something that was never coming.
    let started = std::time::Instant::now();
    let dialled = loop {
        if listener.accept().is_ok() {
            break true;
        }
        if child.try_wait().expect("the child is waitable").is_some() {
            // A connection opened just before the exit is sitting in the accept
            // queue, so the answer is only settled after one more look.
            break dialled_within(&listener, std::time::Duration::ZERO);
        }
        // AND THE POLL HAS A CEILING, because its two exits are a dial and an
        // exit: a child that manages neither would otherwise park the whole
        // suite until somebody killed the runner by hand. The cases here
        // finish in under a second, so nothing legitimate is anywhere near
        // this, and a suite that reports a named failure is worth more than
        // one that hangs.
        if started.elapsed() >= LAMP_DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{name}: the engine neither dialled nor exited within {LAMP_DEADLINE:?}");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let status = child
        .wait_with_output()
        .expect("the child is waitable")
        .status;
    (
        dialled,
        sandbox.fired("mobile"),
        sandbox.fired("hermes"),
        sandbox.fired("macos-banner"),
        status.code(),
    )
}

/// The ceiling on one lamp case, and it is a SUITE SAFETY NET rather than a
/// measurement: these cases finish in well under a second, so a child anywhere
/// near this has stopped making progress.
pub(super) const LAMP_DEADLINE: std::time::Duration = std::time::Duration::from_secs(15);

/// Where the operator is when the event lands, which decides which OTHER leg
/// fires.
///
/// THEY ARE MUTUALLY EXCLUSIVE BY THE SURFACE MODEL: away is a card and no
/// banner, the desk with the pane out of sight is a banner and no card. So
/// showing that a lights mute leaves everything else alone takes one run of
/// each rather than one run asserting all three legs at once.
#[derive(Debug, Clone, Copy)]
pub(super) enum Presence {
    Away,
    Desk,
}

/// Which mute, if any, is typed before the event, which is `lamp_run`'s mute
/// argument: a bare `true` at a call site says nothing about what it decides,
/// and there are now two mutes with deliberately different reaches.
#[derive(Debug, Clone, Copy)]
pub(super) enum Mute {
    Nothing,
    /// `pns quiet 1h`: the whole engine, cards included.
    Everything,
    /// `pns lights quiet <place> 1h`: that place's lamps and nothing else.
    Lights(&'static str),
}

/// A long-running `done`: the event that has earned a pulse since the bash.
pub(super) const LONG_DONE: [&str; 8] = [
    "--agent", "claude", "--state", "done", "--detail", "x", "--pane", "t1:p2",
];

/// A `blocked` turn: an agent waiting on the operator, which earns no pulse on
/// main at any length.
pub(super) const BLOCKED: [&str; 8] = [
    "--agent", "claude", "--state", "blocked", "--detail", "x", "--pane", "t1:p2",
];
