use super::*;

// --- the home probe's diagnostic --------------------------------------------

/// The same three identifiers, all on one client again.
pub(super) const KEYS_AGREE: &str = r#"{"data":[
    {"name":"mister-2","ipAddress":"192.168.1.248","macAddress":"2e:11:ab:6d:b0:4f"}]}"#;

/// A complete listing carrying none of the configured identifiers: the device
/// is out of the house, which is the most ordinary reading this probe takes.
pub(super) const KEYS_AWAY: &str = r#"{"data":[
    {"name":"mouse","ipAddress":"192.168.1.3","macAddress":"60:82:46:3c:fb:01"}]}"#;

/// The router's captive-portal page, which is every unreachable or unreadable
/// answer this probe can get.
pub(super) const NO_LISTING: &str = "<html>router login</html>";

/// That table with a channel to deliver on: the probe now RAISES the stale
/// warning as well as printing it, and a config selecting only the sensor
/// would plan no legs at all, so every one of these would pass without ever
/// exercising a delivery.
pub(super) fn stale_config(router_url: &str) -> String {
    format!(
        "[plugins.hermes]\nenabled = true\n{}",
        router_table(router_url)
    )
}

/// The engine reading the home probe, in the ONLY environment these may run
/// in.
///
/// THE DIAGNOSTIC DELIVERS NOW, so `bare()` is no longer safe here: it reaches
/// the native plugins, walks the developer's own presence probes and, with a
/// hermes key in the config, posts to the operator's REAL gateway. `pns()`
/// points every channel at a recording stub and pins the presence readings,
/// and the URL is pinned at a port nothing listens on as well, so no path
/// through this file can resolve hermes to the live gateway.
pub(super) fn home_probe(sandbox: &Sandbox) -> std::process::Command {
    let mut probe = sandbox.pns();
    probe.env("PNS_HERMES_URL", "http://127.0.0.1:1/webhooks/nowhere");
    probe.arg("home");
    probe
}

/// The hermes stub replaced by one that APPENDS a line per delivery.
///
/// ONE ALERT PER EPISODE IS A COUNT, and the shared stub overwrites: a second
/// delivery would be indistinguishable from the first, and a run that
/// delivered nothing at all still leaves the previous run's file sitting
/// there reading as a delivery.
pub(super) fn count_alerts(sandbox: &Sandbox) {
    sandbox.stub_channel(
        "hermes",
        &format!("cat >>\"{}/hermes.events\"", sandbox.display()),
    );
}

/// Every alert delivered so far, parsed, in order. The engine terminates each
/// event with a newline, so one delivery is one line.
pub(super) fn alerts(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    std::fs::read_to_string(sandbox.path("hermes.events"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).expect("one delivered event per line"))
        .collect()
}

pub(super) const STALE_EVIDENCE: &str = "home: on the home network (matched by device_mac \"2e:11:ab:6d:b0:4f\")\n\
     home:   device_mac \"2e:11:ab:6d:b0:4f\" matched the client the verdict names\n\
     home:   device_hostname \"mister-2\" matched no client\n\
     home:   device_ipv4 \"192.168.1.248\" matched a different client \"mouse\"";

pub(super) const STALE_WARNING: &str =
    "home: an identifier looks stale: device_hostname, device_ipv4 disagree with device_mac";

/// The same disagreement, with the OTHER client named in text nobody here
/// typed: a quote and an ANSI screen clear, straight out of the router.
pub(super) const KEYS_DISAGREE_HOSTILE_LABEL: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;
