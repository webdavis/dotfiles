fn lights_report(
    lights: Option<&pns_domain::lamps::config::Lights>,
    table: Option<&toml::Table>,
    declared: bool,
) -> pns_domain::doctor::LightsReport {
    pns_application::doctor_lamps(lights, || super::doctor_bridge(table, declared))
}
mod tests {
    use super::lights_report;

    #[test]
    fn a_hue_table_nobody_wrote_and_one_switched_off_are_different_reports() {
        // NO BRIDGE IS DIALLED BY ANY ROW HERE: every case answers before the
        // enabled-and-configured branch that makes the two GETs, which is the
        // only branch that touches a network.
        let lights = pns_domain::lamps::config::Lights::default();
        assert!(
            matches!(
                lights_report(None, None, false),
                pns_domain::doctor::LightsReport::Off
            ),
            "no [lights] table is off, whatever hue is doing"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, false),
                pns_domain::doctor::LightsReport::HueMissing
            ),
            "a table and NO [plugins.hue] at all is a config that is half written"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, true),
                pns_domain::doctor::LightsReport::HueDisabled
            ),
            "and a table beside a hue that IS written is a switch somebody turned \
             off, which is a decision rather than an omission"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), Some(&toml::Table::new()), true),
                pns_domain::doctor::LightsReport::NoBridge
            ),
            "an enabled hue naming no bridge dials nothing and says so"
        );
    }
}
