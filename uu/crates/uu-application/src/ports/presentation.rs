use super::{AlertTarget, MarkerSnapshot, StateWriteFailure, StreakKind};
use uu_domain::LaneReport;

pub struct RunHeader {
    pub host: String,
    pub started_iso: String,
    pub gap: String,
}

pub enum Notice<'a> {
    NoAlerts {
        target: AlertTarget<'a>,
        summary: &'a str,
    },
    AlertFailed {
        target: AlertTarget<'a>,
        cause: &'a str,
    },
    NoRecords,
    SigningFailed,
    RecordPosted(&'a str),
    MarkerClockFailed(&'a str),
    MarkerWriteFailed(&'a StateWriteFailure),
    StreakWriteFailed {
        kind: StreakKind,
        lane: &'a str,
        failure: &'a StateWriteFailure,
    },
}

pub trait RunPresentation {
    fn header(&self, epoch: i64, marker: &MarkerSnapshot) -> RunHeader;
    fn write_record(&self, header: &RunHeader, reports: &[LaneReport]) -> String;
    fn notice(&self, notice: Notice<'_>);
}
