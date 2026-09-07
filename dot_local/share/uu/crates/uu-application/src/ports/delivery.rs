#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlertTarget<'a> {
    Run,
    Lane(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub enum AlertOutcome {
    NotConfigured,
    Delivered,
    Failed(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecordFailure {
    Status(u16),
    NoResponse,
    NoStatus,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RecordOutcome {
    NotConfigured,
    SigningFailed,
    Delivered {
        description: String,
    },
    Rejected {
        url: String,
        cause: RecordFailure,
        description: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlarmKind {
    Failed,
    Stale,
    Pending,
    RecordLost,
}

pub struct RunRecord<'a> {
    pub host: &'a str,
    pub failures: usize,
    pub deferred: usize,
    pub pending: usize,
    pub detail: &'a str,
}

pub trait RunDelivery {
    fn alert(
        &self,
        kind: AlarmKind,
        host: &str,
        target: AlertTarget<'_>,
        summary: &str,
    ) -> AlertOutcome;
    fn record(&self, record: RunRecord<'_>) -> RecordOutcome;
}
