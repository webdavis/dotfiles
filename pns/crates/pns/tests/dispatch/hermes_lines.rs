use super::*;

// --- the durable log's printed outcomes -------------------------------------

/// Every hermes outcome an event can reach WITHOUT a live gateway, byte for
/// byte.
///
/// The proof that moving the `pns: ` prefix off the four sentences and onto the
/// one print site changed nothing an operator reads. `tests/native.rs` pins the
/// two outcomes that need a gateway to answer (200 and 401) against the capture
/// server; these are the three that need nothing listening, so between them the
/// set is complete.
#[test]
fn every_hermes_outcome_an_event_can_reach_prints_exactly_what_it_printed_before() {
    for (case, config, url, expected) in [
        (
            "no key in the config",
            "[plugins.log]\nenabled = true\ntype = \"hermes\"\n",
            "http://127.0.0.1:1/hook",
            "pns: post SKIPPED, no hermes key for the pns-events route \
             ([plugins.log.keys] pns-events); nothing was sent\n",
        ),
        (
            "a gateway nothing is listening for",
            "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"k\" }\n",
            "http://127.0.0.1:1/hook",
            "pns: post FAILED HTTP 000 (no response; is the hermes gateway up?)\n",
        ),
        (
            "a url that is never put on the wire",
            "[plugins.log]\nenabled = true\ntype = \"hermes\"\nkeys = { pns-events = \"k\" }\n",
            "http://[::1",
            "pns: post FAILED (curl reported no HTTP status at all)\n",
        ),
    ] {
        let sandbox = Sandbox::new("hermes-outcome-lines");
        sandbox.write_config(config);
        let mut command = sandbox.bare();
        command.env("PNS_HERMES_URL", url);
        // hermes is the only channel here and it never delivers.
        let output = run_expecting(
            1,
            command
                .args([
                    "send",
                    "--producer",
                    "weekly",
                    "--state",
                    "done",
                    "--detail",
                    "ran",
                ])
                .args(["--scope", "remote_only"]),
        );
        assert_eq!(stdout(&output), expected, "case: {case}");
    }
}
