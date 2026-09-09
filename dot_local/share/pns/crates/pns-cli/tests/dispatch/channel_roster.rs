use super::*;

#[test]
fn the_binarys_own_roster_knows_the_router_sensor() {
    // The composition root registers the SAME roster the library's tests run
    // against, so `[plugins.router]` is a known plugin to the real binary. A
    // registry built separately in main would call the operator's correct
    // spelling a typo, warn, and fall back to every built-in, which is how a
    // deliberate selection turns into a delivery nobody asked for.
    let sandbox = Sandbox::new("roster-knows-router");
    // A RECORDING stub under the sensor's own name, so a router registered as
    // a channel has something to reach and leaves a trace. Without it the
    // rogue leg execs a channel script that does not exist, the engine
    // shrugs at a missing channel, and every assertion below still passes.
    sandbox.stub_channel(
        "router",
        &format!("cat >\"{}/router.event\"", sandbox.display()),
    );
    sandbox.write_config(
        "[plugins.router]\nenabled = true\ntype = \"unifi\"\n[plugins.hermes]\nenabled = true\n",
    );
    let output = run(sandbox
        .pns()
        .args(["--agent", "claude", "--state", "done", "--detail", "x"]));
    assert!(
        !stderr(&output).contains("unknown plugin"),
        "the sensor is registered, not a typo: {output:?}"
    );
    assert!(sandbox.fired("hermes"), "the selection still delivers");
    assert!(
        !sandbox.fired("mobile"),
        "and nothing fell back to the whole roster"
    );
    assert!(
        !sandbox.fired("router"),
        "the roster registers router as a SENSOR: an input carries no routing, \
         so no event can be delivered to it"
    );
}
