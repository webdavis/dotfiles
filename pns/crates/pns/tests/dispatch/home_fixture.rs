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

/// The whole diagnostic, in the house style, as `pns home` prints it to a pipe.
///
/// ONE PLACE. Four cases assert the whole of stdout, because "the diagnostic is
/// untouched" is what several of them are actually about, and four copies would
/// be four things to edit whenever the format moves.
///
/// WRITTEN WITHOUT LINE CONTINUATIONS. A `\` at the end of a Rust string eats
/// the leading whitespace of the next line, which is exactly the indentation
/// these rows are being checked for.
pub(super) const STALE_EVIDENCE: &str = concat!(
    "\npns home\n",
    "Looking for   the client [plugins.router] names, on the home network\n",
    "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\n",
    "\n",
    "\u{25c6} Verdict \u{2500}\u{2500} what the router's client list says\n",
    "\n",
    "  \u{2713} on the home network, matched by device_mac \"2e:11:ab:6d:b0:4f\"\n",
    "\n",
    "\u{25c6} Evidence \u{2500}\u{2500} what each configured identifier matched\n",
    "\n",
    "  \u{b7} device_mac        \"2e:11:ab:6d:b0:4f\"   matched the client the verdict names\n",
    "  \u{b7} device_hostname   \"mister-2\"   matched no client\n",
    "  \u{b7} device_ipv4       \"192.168.1.248\"   matched a different client \"mouse\"",
);

/// The warning SENTENCE, which is what the alert body carries.
///
/// NO GLYPH AND NO INDENT. A notification is one sentence going to a channel,
/// not a row in a terminal report, so the two are separate constants: sharing
/// one made an alert-body assertion start expecting a terminal's decoration.
pub(super) const STALE_WARNING: &str = concat!(
    "an identifier looks stale: device_hostname, device_ipv4 ",
    "disagree with device_mac",
);

/// The same sentence as the report's warning ROW, with the blank line above it.
///
/// THE BLANK IS PART OF IT so the stdout call sites can keep writing
/// `format!("{STALE_EVIDENCE}\n{STALE_WARNING_ROW}\n")` unchanged.
pub(super) const STALE_WARNING_ROW: &str = concat!(
    "\n  \u{26a0} an identifier looks stale: device_hostname, device_ipv4 ",
    "disagree with device_mac",
);

/// The same disagreement, with the OTHER client named in text nobody here
/// typed: a quote and an ANSI screen clear, straight out of the router.
pub(super) const KEYS_DISAGREE_HOSTILE_LABEL: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;
