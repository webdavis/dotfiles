mod tests {
    use super::super::*;

    #[test]
    fn a_hue_table_nobody_wrote_and_one_switched_off_are_different_reports() {
        // NO BRIDGE IS DIALLED BY ANY ROW HERE: every case answers before the
        // enabled-and-configured branch that makes the two GETs, which is the
        // only branch that touches a network.
        let lights = pns::config::Lights::default();
        assert!(
            matches!(
                lights_report(None, None, false),
                pns::doctor::LightsReport::Off
            ),
            "no [lights] table is off, whatever hue is doing"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, false),
                pns::doctor::LightsReport::HueMissing
            ),
            "a table and NO [plugins.hue] at all is a config that is half written"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), None, true),
                pns::doctor::LightsReport::HueDisabled
            ),
            "and a table beside a hue that IS written is a switch somebody turned \
             off, which is a decision rather than an omission"
        );
        assert!(
            matches!(
                lights_report(Some(&lights), Some(&toml::Table::new()), true),
                pns::doctor::LightsReport::NoBridge
            ),
            "an enabled hue naming no bridge dials nothing and says so"
        );
    }
}
