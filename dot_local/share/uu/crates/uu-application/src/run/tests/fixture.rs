use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Event {
    Acquire,
    Release,
    Prune(Vec<String>),
    Epoch(Option<i64>),
    MarkerRead,
    Header(i64, Marker),
    Start,
    Elapsed(Duration),
    Lane(String, Duration, Duration, i64, String, Marker),
    Render(Vec<LaneReport>),
    StreakRead(StreakKind, String),
    StreakWrite(StreakKind, String, u32),
    Alert(AlarmKind, String),
    Record(usize, usize, usize, String),
    Notice(String),
    MarkerWrite(i64),
}

pub(super) struct Data {
    pub events: Vec<Event>,
    pub held: bool,
    pub lock_failure: Option<LockFailure>,
    pub epochs: VecDeque<Result<i64, ClockFailure>>,
    pub elapsed: VecDeque<Duration>,
    pub reports: BTreeMap<String, LaneReport>,
    pub streaks: BTreeMap<String, u32>,
    pub pending: BTreeMap<String, u32>,
    pub unreadable_pending: bool,
    pub fail_pending_write: bool,
    pub summaries: Vec<String>,
    pub fail_alerts: bool,
    pub record: Option<RecordOutcome>,
}

#[derive(Clone)]
pub(super) struct Fixture(pub Rc<RefCell<Data>>);

impl Fixture {
    pub fn new(reports: Vec<LaneReport>) -> Self {
        Self(Rc::new(RefCell::new(Data {
            events: Vec::new(),
            held: false,
            lock_failure: None,
            epochs: VecDeque::from([Ok(100), Ok(200)]),
            elapsed: VecDeque::from(vec![Duration::ZERO; reports.len()]),
            reports: reports
                .into_iter()
                .map(|report| (report.name.clone(), report))
                .collect(),
            streaks: BTreeMap::new(),
            pending: BTreeMap::new(),
            unreadable_pending: false,
            fail_pending_write: false,
            summaries: Vec::new(),
            fail_alerts: false,
            record: Some(RecordOutcome::Delivered {
                description: "posted".into(),
            }),
        })))
    }

    pub fn run(&self, lanes: &[(&str, u64)], only: Option<&str>) -> RunOutcome {
        self.run_with_threshold(lanes, only, uu_domain::DEFAULT_ESCALATE_AFTER_RUNS)
    }

    pub fn run_with_threshold(
        &self,
        lanes: &[(&str, u64)],
        only: Option<&str>,
        threshold: std::num::NonZeroU32,
    ) -> RunOutcome {
        let lanes = lanes
            .iter()
            .map(|(name, secs)| {
                (
                    name.to_string(),
                    LaneSettings {
                        deadline: Duration::from_secs(*secs),
                        escalate_after_runs: threshold,
                    },
                )
            })
            .collect();
        Run {
            state: self.clone(),
            clock: self.clone(),
            lanes: self.clone(),
            delivery: self.clone(),
            presentation: self.clone(),
        }
        .execute(RunRequest {
            lanes: &lanes,
            only,
        })
    }

    pub fn events(&self) -> Vec<Event> {
        self.0.borrow().events.clone()
    }

    pub fn event(&self, event: Event) {
        let mut data = self.0.borrow_mut();
        assert!(data.held, "operation without the run guard: {event:?}");
        data.events.push(event);
    }
}

mod state;

impl RunClock for Fixture {
    type Tick = u8;
    fn epoch(&self) -> Result<i64, ClockFailure> {
        let result = self
            .0
            .borrow_mut()
            .epochs
            .pop_front()
            .expect("unexpected clock sample");
        self.event(Event::Epoch(result.as_ref().ok().copied()));
        result
    }
    fn start(&self) -> u8 {
        self.event(Event::Start);
        7
    }
    fn elapsed(&self, tick: &u8) -> Duration {
        assert_eq!(*tick, 7);
        let elapsed = self
            .0
            .borrow_mut()
            .elapsed
            .pop_front()
            .expect("unexpected elapsed sample");
        self.event(Event::Elapsed(elapsed));
        elapsed
    }
}

impl LaneExecutor for Fixture {
    fn execute(
        &self,
        name: &str,
        budget: Duration,
        deadline: Duration,
        facts: &RunFacts<'_>,
    ) -> LaneExecution {
        assert_eq!(facts.host, "fixture-host");
        self.event(Event::Lane(
            name.into(),
            budget,
            deadline,
            facts.started_epoch,
            facts.started_iso.into(),
            facts.marker.clone(),
        ));
        LaneExecution::Reported(
            self.0
                .borrow()
                .reports
                .get(name)
                .expect("unexpected lane")
                .clone(),
        )
    }
}

impl RunDelivery for Fixture {
    fn alert(
        &self,
        kind: AlarmKind,
        host: &str,
        target: AlertTarget<'_>,
        summary: &str,
    ) -> AlertOutcome {
        assert_eq!(host, "fixture-host");
        self.0.borrow_mut().summaries.push(summary.into());
        self.event(Event::Alert(
            kind,
            match target {
                AlertTarget::Run => "run".into(),
                AlertTarget::Lane(name) => name.into(),
            },
        ));
        if self.0.borrow().fail_alerts {
            AlertOutcome::Failed("fixture transport failure".into())
        } else {
            AlertOutcome::Delivered
        }
    }
    fn record(&self, record: RunRecord<'_>) -> RecordOutcome {
        assert_eq!(record.host, "fixture-host");
        self.event(Event::Record(
            record.failures,
            record.deferred,
            record.pending,
            record.detail.into(),
        ));
        self.0.borrow_mut().record.take().expect("only one record")
    }
}

impl RunPresentation for Fixture {
    fn header(&self, epoch: i64, marker: &MarkerSnapshot) -> RunHeader {
        self.event(Event::Header(epoch, marker.value.clone()));
        RunHeader {
            host: "fixture-host".into(),
            started_iso: format!("epoch-{epoch}"),
            gap: "previous gap".into(),
        }
    }
    fn write_record(&self, header: &RunHeader, reports: &[LaneReport]) -> String {
        assert_eq!(header.host, "fixture-host");
        assert_eq!(header.gap, "previous gap");
        self.event(Event::Render(reports.to_vec()));
        "fixture record body".into()
    }
    fn notice(&self, notice: Notice<'_>) {
        let text = match notice {
            Notice::NoAlerts { .. } => "no alerts".into(),
            Notice::AlertFailed { .. } => "alert failed".into(),
            Notice::NoRecords => "no records".into(),
            Notice::SigningFailed => "signing failed".into(),
            Notice::RecordPosted(description) => description.into(),
            Notice::MarkerClockFailed(path) => format!("clock failed: {path}"),
            Notice::MarkerWriteFailed(_) => panic!("fixture marker writes succeed"),
            Notice::StreakWriteFailed { kind, failure, .. } => {
                format!("{kind:?} streak write failed: {}", failure.cause)
            }
        };
        self.event(Event::Notice(text));
    }
}
