use super::*;
use crate::{ControlRecord, ControlValue, ControlsInput, validate_controls};
mod baseline;
mod classify;
mod gap;
mod transitions;
fn control(id: &str, expect: &str, target: &str) -> Control {
    validate_controls(ControlsInput::Records(&[ControlRecord {
        id,
        tier: "verify",
        reader: if target.is_empty() {
            "fdesetup_status"
        } else {
            "lulu_rule_present"
        },
        expect,
        target,
        description: id,
        remedy: "check it",
    }]))
    .unwrap()
    .remove(0)
}
fn trio<'a>(values: [&'a str; 3]) -> TrioReading<'a> {
    TrioReading { values, exit: 0 }
}
fn previous<'a>(values: [&str; 3], controls: &'a [ControlPrior<'a>]) -> PollBaseline<'a> {
    trusted_poll_baseline(Some(0o600), true, values, controls).unwrap()
}
fn observe(control: &Control, value: ControlValue) -> ControlObservation<'_> {
    ControlObservation {
        control,
        reading: ControlReading::Known(value),
    }
}
