#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmFailed;

pub trait IndependentAlarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed>;
}
