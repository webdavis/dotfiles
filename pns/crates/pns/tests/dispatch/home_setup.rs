use super::*;

#[test]
fn every_way_the_home_probe_is_not_set_up_says_which_one_it_is() {
    // `home_mode` is the ONE place a cause becomes the line an operator
    // reads, and that wiring only runs through the binary: collapsing its two
    // failure arms onto a single message left every other test green, so a
    // disabled probe and a type nothing answers could both print "no
    // api_key" with nothing to say so. Exact lines, because a cause that
    // merely contains "home:" sends the operator to the wrong edit.
    let sandbox = Sandbox::without_config("home-setup-failures");
    // Every case here stops before the router is read, so none of them can
    // reach a delivery; it still runs in the safe environment, because "this
    // path happens not to dispatch today" is not a property a test should be
    // relying on to stay off the operator's real gateway.
    let home_line = || {
        let mut probe = home_probe(&sandbox);
        stdout(&run(&mut probe)).trim_end().to_string()
    };
    // No config has been written yet, so this case has to come first.
    assert_eq!(home_line(), "home: not configured (no config file)");
    for (config, line) in [
        (
            // The retired feature table, refused by NAME rather than ignored,
            // AND the refusal names the tables that do work. This is the one
            // an operator actually meets: `[home]` moved under
            // `[plugins.router]`, so a config written before that move is
            // refused whole, which takes every plugin's secret with it, and
            // "unknown" on its own leaves nowhere to go.
            "[home]\nrouter_url = \"https://192.168.1.1\"\nphone = \"mister\"\n",
            "home: config error (unknown top-level key `home`; the file serves \
             daemon, delivery, failures, focus, lights, nag, plugins, recap)",
        ),
        (
            "[plugins.hermes]\nenabled = true\n",
            "home: not configured (no [plugins.router] table)",
        ),
        (
            "[plugins.router]\nenabled = false\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: [plugins.router] is present but enabled = false",
        ),
        (
            "[plugins.router]\nenabled = true\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: no type in [plugins.router] (the only type is \"unifi\")",
        ),
        (
            "[plugins.router]\nenabled = true\ntype = \"asus\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: [plugins.router] has type \"asus\", which no compiled-in backend answers \
             (the only type is \"unifi\")",
        ),
        (
            // The URL is the one setting left outside the device keys, so it
            // keeps its own line, and that line no longer names them.
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             device_hostname = \"mister\"\napi_key = \"k-123\"\n",
            "home: the [plugins.router] table is present but router_url is missing, empty, \
             or not a string",
        ),
        (
            // A table with no device in it at all: the line names the three
            // keys to set rather than any key that went away, since there is
            // no back-compat here.
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\napi_key = \"k-123\"\n",
            "home: no device to look for in [plugins.router] \
             (set at least one of device_mac, device_hostname, device_ipv4)",
        ),
        (
            // And a config still carrying the retired `phone` key no longer
            // reaches that line at all: the table's own vocabulary is judged
            // at load, so the key that went away is named where the operator
            // wrote it, with the keys that replaced it in the same sentence.
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\nphone = \"mister\"\napi_key = \"k-123\"\n",
            "home: config error (unknown `plugins.router` key `phone`; the table serves \
             api_key, device_hostname, device_ipv4, device_mac, enabled, router_url, \
             stale_alert_channel, type)",
        ),
        (
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_ipv4 = \"192.168.1\"\napi_key = \"k-123\"\n",
            "home: device_ipv4 = \"192.168.1\" in [plugins.router] is not an IPv4 address \
             (a dotted quad, e.g. \"192.168.1.169\")",
        ),
        (
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_mac = \"2e11ab6db04f\"\napi_key = \"k-123\"\n",
            "home: device_mac = \"2e11ab6db04f\" in [plugins.router] is not a MAC address \
             (six hex pairs under one separator, e.g. \"2e:11:ab:6d:b0:4f\")",
        ),
        (
            // Everything else is in order, so the key is the only thing left
            // to be missing, and the probe stops before it reaches a router.
            "[plugins.router]\nenabled = true\ntype = \"unifi\"\n\
             router_url = \"https://192.168.1.1\"\ndevice_hostname = \"mister\"\n",
            "home: no api_key in the [plugins.router] table (the probe is not set up)",
        ),
    ] {
        sandbox.write_config(config);
        assert_eq!(home_line(), line, "case: {config:?}");
    }
}
