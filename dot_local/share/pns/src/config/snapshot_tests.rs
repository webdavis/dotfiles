use super::*;

/// THE MUTANT `LIVE_TABLES` CANNOT SEE (measured, this slice's second
/// review round). `plugins.hue.rooms`, `plugins.hue.quiet_hours`,
/// `plugins.router.router_url` and `plugins.router.device_hostname` all
/// render COMMENTED when dropped from the values file, never touching a
/// table heading, so the live-table count stays 22 and the test above
/// cannot see it: dropping `rooms` or `router_url` removes both from the
/// resolved `Config` outright, the room pulse and the router target
/// simply gone. A fifth key, `lights.loop.threshold_secs`, renders LIVE
/// but at its schema default (300) when dropped, which changes a value
/// the same way as if the operator had typed `300` in the values file
/// themselves. Neither shape moves a table heading, so both need the
/// PARSED config compared, not the template text.
///
/// TO UPDATE AFTER A DELIBERATE CHANGE to `config-values.toml`: run
/// `cargo test -p pns --lib \
/// config::tests::the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot`
/// from `dot_local/share/pns`, note the scratch path the failure names,
/// `diff` that path against `tests/fixtures/resolved-config.snapshot` and
/// read what moved, then `cp` the scratch file over the committed one (the
/// panic prints both commands verbatim) and commit the new snapshot. A
/// snapshot updated without reading that diff is a guard bypassed rather
/// than obeyed.
///
/// THE REMAINING CEILING, measured rather than assumed: this pins the
/// `Debug` text of the whole resolved `Config`, so it catches any change
/// that reaches `Config`, which is every plugin's raw settings table, the
/// four top-level tables and the whole lamp map, and it catches a secret
/// moving to a different entry or field too (`identity_placeholder`
/// stubs each one to a string carrying its own entry and field, so a
/// swapped secret changes the printed text). It does NOT catch a change
/// that never reaches `Config` at all: a plugin's own runtime reading of
/// its `settings` table (`channels/hue.rs` deciding what a room NAME
/// means, for one) is downstream of this layer and out of its reach, the
/// same boundary `config.rs`'s own module doc draws. And it is only as
/// honest as whoever updates it: nothing stops a `cp` run without reading
/// the diff first, which is why the diff step is spelled out above rather
/// than folded into one command.
#[test]
fn the_resolved_configuration_over_the_committed_values_file_matches_its_snapshot() {
    let values: toml::Table = CONFIG_VALUES
        .parse()
        .expect("the committed values file is valid TOML");
    let rendered = crate::config_text::render(&values).expect("the committed values file renders");
    let stripped = super::strip_chezmoi_actions(&rendered, super::identity_placeholder)
        .expect("render's own output is well-formed");
    let config = parse_config(&stripped)
        .unwrap_or_else(|error| panic!("the rendered config must load: {error:?}"));
    let actual = format!("{config:#?}\n");

    if actual == RESOLVED_CONFIG_SNAPSHOT {
        return;
    }
    let scratch = std::env::temp_dir().join(format!(
        "pns-resolved-config-{}.snapshot",
        std::process::id()
    ));
    std::fs::write(&scratch, &actual).ok();
    let scratch = scratch.display();
    panic!(
        "the resolved configuration drifted from the committed snapshot; run `diff \
             {RESOLVED_CONFIG_SNAPSHOT_PATH} {scratch}` to see exactly what changed. If \
             `dot_config/pns/config-values.toml` changed on purpose, after reading that diff run \
             `cp {scratch} {RESOLVED_CONFIG_SNAPSHOT_PATH}` and commit the new snapshot."
    );
}
