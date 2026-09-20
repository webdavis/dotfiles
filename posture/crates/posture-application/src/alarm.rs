#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmFailed;

pub trait IndependentAlarm {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed>;
}

/// A borrowed alarm IS an alarm, so a job that must still report on its own
/// after handing the banner to a sink can lend it rather than build a second.
impl<A: IndependentAlarm + ?Sized> IndependentAlarm for &mut A {
    fn alarm(&mut self, title: &str, detail: &str) -> Result<(), AlarmFailed> {
        (**self).alarm(title, detail)
    }
}
