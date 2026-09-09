use crate::ControlValue;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trio(pub(super) [u8; 3]);
impl Trio {
    pub fn values(self) -> [u8; 3] {
        self.0
    }
}
#[derive(Debug, Clone, Copy)]
pub struct TrioReading<'a> {
    pub values: [&'a str; 3],
    pub exit: i32,
}
#[derive(Debug, Clone, Copy)]
pub struct ControlPrior<'a> {
    pub id: &'a str,
    pub value: &'a str,
    pub expect: &'a str,
    pub target: &'a str,
}
#[derive(Debug, Clone, Copy)]
pub struct PollBaseline<'a> {
    pub(super) trio: Trio,
    pub(super) controls: &'a [ControlPrior<'a>],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredControl {
    pub id: String,
    pub value: ControlValue,
    pub expect: ControlValue,
    pub target: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineUpdate {
    pub trio: Trio,
    pub controls: Vec<StoredControl>,
    pub preserve_prior_fields: bool,
}
pub fn trusted_poll_baseline<'a>(
    mode: Option<u32>,
    one_object: bool,
    values: [&str; 3],
    controls: &'a [ControlPrior<'a>],
) -> Option<PollBaseline<'a>> {
    if mode != Some(0o600) || !one_object {
        return None;
    }
    Some(PollBaseline {
        trio: read_trio(TrioReading { values, exit: 0 })?,
        controls,
    })
}

pub(super) fn read_trio(reading: TrioReading<'_>) -> Option<Trio> {
    if reading.exit != 0 {
        return None;
    }
    let [fw, gk, sl] = reading.values;
    if !["0", "1", "2"].contains(&fw) || !["0", "1"].contains(&gk) || !["0", "1"].contains(&sl) {
        return None;
    }
    Some(Trio([
        fw.as_bytes()[0] - b'0',
        gk.as_bytes()[0] - b'0',
        sl.as_bytes()[0] - b'0',
    ]))
}

pub(super) fn prior_value(
    prior: Option<PollBaseline<'_>>,
    control: &crate::Control,
) -> Option<ControlValue> {
    let saved = prior?
        .controls
        .iter()
        .find(|saved| saved.id == control.id)?;
    if saved.expect != control.expect.as_str() || saved.target != control.target {
        return None;
    }
    control.reader.value(saved.value)
}

pub(super) fn stored(control: &crate::Control, value: ControlValue) -> StoredControl {
    StoredControl {
        id: control.id.clone(),
        value,
        expect: control.expect,
        target: control.target.clone(),
    }
}
