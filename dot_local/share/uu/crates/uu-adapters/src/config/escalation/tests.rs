use crate::config::probes::{parsed, refusal};

#[test]
fn escalate_after_runs_defaults_to_three_on_every_lane_type() {
    for (name, text) in [
        ("brew", "[lanes.brew]\n"),
        ("herdr", "[lanes.herdr]\n"),
        ("npm", "[lanes.npm]\nbinary = \"/fixture/npm\"\n"),
        ("uv", "[lanes.uv]\n"),
        ("command", "[lanes.command]\nrun = [\"updater\"]\n"),
    ] {
        assert_eq!(
            parsed(text).lanes[name].escalate_after_runs.get(),
            3,
            "{name}"
        );
    }
}

#[test]
fn escalation_accepts_both_positive_integer_bounds_and_custom_values() {
    for value in [1, 2, u32::MAX] {
        let config = parsed(&format!(
            "[lanes.mine]\ntype = \"command\"\nrun = [\"updater\"]\nescalate_after_runs = {value}\n"
        ));
        assert_eq!(config.lanes["mine"].escalate_after_runs.get(), value);
    }
}

#[test]
fn invalid_escalation_values_are_refused_by_key_and_lane_name() {
    for value in ["0", "-1", "4294967296", "1.5", "true", "\"three\"", "[]"] {
        let why = refusal(&format!(
            "[lanes.mine]\ntype = \"command\"\nrun = [\"updater\"]\nescalate_after_runs = {value}\n"
        ));
        assert!(
            why.contains("lanes.mine")
                && why.contains("escalate_after_runs")
                && why.contains("positive whole"),
            "{value}: {why}"
        );
    }
}
