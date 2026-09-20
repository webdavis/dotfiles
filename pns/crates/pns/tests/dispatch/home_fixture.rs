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
        "[plugins.log]\nenabled = true\ntype = \"hermes\"\n{}",
        router_table(router_url)
    )
}

/// The engine reading the home probe, which is `pns doctor` now, in the ONLY
/// environment these may run in.
///
/// THE REPORT DELIVERS, so `bare()` is not safe here: it reaches the native
/// plugins, walks the developer's own presence probes and, with a hermes key
/// in the config, posts to the operator's REAL gateway. `pns()` points every
/// channel at a recording stub and pins the presence readings, and the URL is
/// pinned at a port nothing listens on as well, so no path through this file
/// can resolve hermes to the live gateway. The moshi-hook path is stubbed
/// absent for the reason `doctor_command` states.
///
/// THE STATE DIRECTORY IS LEFT AT ITS DEFAULT, unlike `doctor_command`'s, so
/// the staleness memory these cases read lands under the sandbox's own
/// `.local/state/pns`.
pub(super) fn home_probe(sandbox: &Sandbox) -> std::process::Command {
    let mut probe = sandbox.pns();
    probe.env("PNS_HERMES_URL", "http://127.0.0.1:1/webhooks/nowhere");
    no_moshi_hook(sandbox, &mut probe);
    probe.arg("doctor");
    probe
}

/// The home probe's own rows out of the whole report: the verdict row and
/// everything printed under it before the next section.
///
/// THE WHOLE REPORT IS NOT WHAT THESE CASES ARE ABOUT. `pns doctor` prints a
/// dozen rows about channels, pairing and the ledger that move on their own
/// schedule, and asserting all of them here would make every home case a
/// hostage to an unrelated section.
pub(super) fn home_rows(reported: &str) -> Vec<String> {
    let mut rows = Vec::new();
    for line in reported.lines().map(str::trim_start) {
        if line.starts_with('\u{25c6}') && !rows.is_empty() {
            break;
        }
        let row = [
            "\u{2713} ",
            "\u{2717} ",
            "\u{26a0} ",
            "\u{b7} ",
            "\u{2192} ",
        ]
        .iter()
        .find_map(|glyph| line.strip_prefix(glyph));
        match row {
            Some(row) if row.starts_with("home: ") => rows.push(row.to_string()),
            Some(row) if !rows.is_empty() => rows.push(row.to_string()),
            _ => {}
        }
    }
    rows
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

/// Every STALE ALERT delivered so far, parsed, in order. The engine terminates
/// each event with a newline, so one delivery is one line.
///
/// THE DOCTOR'S OWN TEST SEND RIDES THE SAME STUB, so the state word is what
/// tells the two apart: counting every line would count one test notification
/// per run as an alert about the identifiers.
pub(super) fn alerts(sandbox: &Sandbox) -> Vec<serde_json::Value> {
    std::fs::read_to_string(sandbox.path("hermes.events"))
        .unwrap_or_default()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line).expect("one delivered event per line")
        })
        .filter(|event| event["state"] == "stale")
        .collect()
}

/// The home probe's rows, as the report prints them for the operator's own
/// stale case.
///
/// ONE PLACE. Several cases assert the whole block, because "the probe's rows
/// are untouched" is what they are actually about, and four copies would be
/// four things to edit whenever the wording moves.
pub(super) fn stale_evidence() -> Vec<String> {
    [
        "home: on the home network, matched by device_mac \"2e:11:ab:6d:b0:4f\"",
        "device_mac        \"2e:11:ab:6d:b0:4f\"   matched the client the verdict names",
        "device_hostname   \"mister-2\"   matched no client",
        "device_ipv4       \"192.168.1.248\"   matched a different client \"mouse\"",
    ]
    .iter()
    .map(|row| (*row).to_string())
    .collect()
}

/// The same rows with the staleness warning under them, which is what a run
/// that has news to tell prints.
pub(super) fn stale_evidence_warned() -> Vec<String> {
    let mut rows = stale_evidence();
    rows.push(STALE_WARNING.to_string());
    rows
}

/// The warning SENTENCE, which is both the report's warning row and the alert
/// body: `stale_warning` is the one place it is written.
pub(super) const STALE_WARNING: &str = concat!(
    "an identifier looks stale: device_hostname, device_ipv4 ",
    "disagree with device_mac",
);

/// The same disagreement, with the OTHER client named in text nobody here
/// typed: a quote and an ANSI screen clear, straight out of the router.
pub(super) const KEYS_DISAGREE_HOSTILE_LABEL: &str = r#"{"data":[
    {"name":"mister","ipAddress":"192.168.1.169","macAddress":"2e:11:ab:6d:b0:4f"},
    {"name":"mo\"use\u001b[2J","ipAddress":"192.168.1.248","macAddress":"60:82:46:3c:fb:01"}]}"#;

/// The same rows with each substitution applied, for a case that differs from
/// the operator's own stale reading in one or two sentences.
pub(super) fn rewritten(rows: &[String], edits: &[(&str, &str)]) -> Vec<String> {
    rows.iter()
        .map(|row| {
            edits
                .iter()
                .fold(row.clone(), |row, (from, to)| row.replace(from, to))
        })
        .collect()
}
