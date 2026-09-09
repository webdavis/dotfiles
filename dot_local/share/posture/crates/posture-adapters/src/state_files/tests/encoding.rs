use super::*;
use posture_domain::{
    ControlObservation, ControlReading, ControlRecord, ControlsInput, ControlsRead, LuluProfile,
    plan_poll, validate_controls,
};
fn controls(case: &serde_json::Value) -> Vec<Control> {
    case["controls"].as_array().map_or_else(Vec::new, |rows| {
        rows.iter()
            .flat_map(|row| {
                validate_controls(ControlsInput::Records(&[ControlRecord {
                    id: row[0].as_str().unwrap(),
                    tier: "verify",
                    reader: if row[3] == "" {
                        "fdesetup_status"
                    } else {
                        "lulu_rule_present"
                    },
                    expect: row[2].as_str().unwrap(),
                    target: row[3].as_str().unwrap(),
                    description: "fixture",
                    remedy: "",
                }]))
                .unwrap()
            })
            .collect()
    })
}
fn query(dir: &Path, bytes: &str) -> crate::PostureTrio {
    let program = dir.join("query");
    put(
        &program,
        &format!(
            "#!/bin/sh\nprintf '%s' '{}'\n",
            bytes.replace('\'', "'\\''")
        ),
        0o700,
    );
    crate::PostureQuery::new(program).read().unwrap()
}
#[test]
fn published_baselines_preserve_captured_rows_prior_fields_and_control_declarations() {
    let cases: Vec<serde_json::Value> = serde_json::from_str(include_str!("fold.json")).unwrap();
    for case in cases {
        let dir = root();
        let mut store = PollStateFiles::new(dir.join("state"));
        if let Some(prior) = case["prior"].as_str() {
            put(&store.baseline, prior, 0o600);
        }
        let controls = controls(&case);
        let saved = store.read(&controls);
        let priors: Vec<_> = saved
            .as_ref()
            .map_or_else(Vec::new, |s| s.controls.iter().map(|c| c.prior()).collect());
        let prior = saved.as_ref().and_then(|s| s.baseline(&priors));
        let current = query(&dir, case["query"].as_str().unwrap());
        let observations: Vec<_> = controls
            .iter()
            .enumerate()
            .map(|(index, control)| ControlObservation {
                control,
                reading: control
                    .reader()
                    .value(case["controls"][index][1].as_str().unwrap())
                    .map_or(ControlReading::Indeterminate, ControlReading::Known),
            })
            .collect();
        let refusal = validate_controls(ControlsInput::Malformed).unwrap_err();
        let reading = if case["refused"] == true {
            ControlsRead::Refused(&refusal)
        } else {
            ControlsRead::Observed(&observations)
        };
        let plan = plan_poll(current.reading(), reading, prior, &[], LuluProfile::Base);
        store.publish(&plan.baseline.unwrap(), &current).unwrap();
        assert_eq!(
            fs::read_to_string(&store.baseline).unwrap(),
            format!("{}\n", case["output"].as_str().unwrap()),
            "{}",
            case["name"]
        );
        assert_eq!(modes(&store.baseline), 0o600);
        assert!(!store.sibling(".tmp").exists());
    }
}
