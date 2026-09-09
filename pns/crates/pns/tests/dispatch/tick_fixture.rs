use super::*;

// --- the lamps' tick job ----------------------------------------------------

/// The tick job the event path registered, or a panic naming what was there
/// instead.
pub(super) fn lights_job(sandbox: &Sandbox) -> pns_domain::jobs::Job {
    let record = std::fs::read_to_string(sandbox.path("state/daemon/lights"))
        .expect("the event registered no lights job");
    pns_adapters::job_spool::parse(record.trim_end_matches('\n')).expect("a job record")
}

/// One event against a sandbox whose lamps are mapped, with no bridge to
/// reach: the registration is what these are about, and it takes no network.
pub(super) fn registering_event(name: &str) -> Sandbox {
    let sandbox = Sandbox::new(name);
    sandbox.write_config(&format!(
        "[plugins.hue]\nenabled = true\n[plugins.mobile]\nenabled = true\ntype = \"moshi\"\n\
         [plugins.hermes]\nenabled = true\n{STUDIO_MAP}"
    ));
    sandbox
}

// --- the tick ---------------------------------------------------------------

/// A loopback port with nothing listening on it, so a bridge call is refused at
/// once rather than waiting out the ten-second transport deadline.
///
/// BOUND THEN DROPPED, which is how a port is known to be free without holding
/// it: a listener that stayed open would queue the connection and the TLS
/// handshake would sit there until the deadline.
pub(super) fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("listener");
    listener.local_addr().expect("addr").port()
}

/// One `pns lights tick` against this sandbox.
///
/// `herdr` IS STUBBED, WITHOUT EXCEPTION. The tick reads `workspace list` for
/// the working aggregate, and the binary it resolves through PATH is the
/// developer's own multiplexer: unstubbed, these tests would read whatever the
/// operator happens to be running and answer differently on every machine and
/// every run. The shipped stub carries no `agent_status`, which is the
/// not-working reading.
pub(super) fn tick(sandbox: &Sandbox) -> std::process::Output {
    let mut command = logged_event(sandbox);
    sandbox.stub_herdr(&mut command, false);
    command
        .args(["lights", "tick"])
        .output()
        .expect("the engine runs")
}

/// A session waiting on the operator, planted so the tick has a state to arm
/// and really reaches for the bridge.
pub(super) fn plant_waiting_session(sandbox: &Sandbox) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs();
    let needs = sandbox.path("state/lights-blocked");
    std::fs::create_dir_all(&needs).expect("the needs directory");
    std::fs::write(needs.join("s1"), format!("{now}\n")).expect("a needs marker");
}

/// The lights job the spool is holding, as its raw record, or nothing.
pub(super) fn scheduled_tick(sandbox: &Sandbox) -> Option<String> {
    std::fs::read_to_string(sandbox.path("state/daemon/lights")).ok()
}

/// The second a spool record stops being allowed to run.
pub(super) fn lease_ends_at(record: &str) -> u64 {
    record
        .split_whitespace()
        .find_map(|field| field.strip_prefix("until="))
        .and_then(|until| until.parse().ok())
        .unwrap_or_else(|| panic!("the record states an until: {record:?}"))
}

pub(super) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("a clock past 1970")
        .as_secs()
}
