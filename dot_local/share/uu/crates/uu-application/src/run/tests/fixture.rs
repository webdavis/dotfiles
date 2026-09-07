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
    StreakRead(String),
    StreakWrite(String, u32),
    Alert(String),
    Record(usize, usize, String),
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
            fail_alerts: false,
            record: Some(RecordOutcome::Delivered {
                description: "posted".into(),
            }),
        })))
    }

    pub fn run(&self, lanes: &[(&str, u64)], only: Option<&str>) -> RunOutcome {
        let lanes = lanes
            .iter()
            .map(|(name, secs)| (name.to_string(), Duration::from_secs(*secs)))
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

pub(super) struct Guard(Fixture);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.event(Event::Release);
        self.0.0.borrow_mut().held = false;
    }
}

impl RunState for Fixture {
    type Guard = Guard;
    fn acquire(&self) -> Result<Guard, LockFailure> {
        let mut data = self.0.borrow_mut();
        assert!(!data.held);
        data.events.push(Event::Acquire);
        if let Some(failure) = data.lock_failure.take() {
            return Err(failure);
        }
        data.held = true;
        Ok(Guard(self.clone()))
    }
    fn prune_removed_lanes(&self, declared: &[&str]) {
        self.event(Event::Prune(
            declared.iter().map(|name| name.to_string()).collect(),
        ));
    }
    fn marker(&self) -> MarkerSnapshot {
        self.event(Event::MarkerRead);
        MarkerSnapshot {
            value: Marker::Recorded {
                epoch: 7,
                iso: "previous".into(),
            },
            location: "/fixture/marker".into(),
        }
    }
    fn write_marker(&self, epoch: i64) -> Result<(), StateWriteFailure> {
        self.event(Event::MarkerWrite(epoch));
        Ok(())
    }
    fn streak(&self, lane: &str) -> StreakSnapshot {
        self.event(Event::StreakRead(lane.into()));
        StreakSnapshot {
            value: self
                .0
                .borrow()
                .streaks
                .get(lane)
                .copied()
                .map_or(Streak::Absent, Streak::Value),
            location: format!("/fixture/{lane}/streak"),
        }
    }
    fn write_streak(&self, lane: &str, value: u32) -> Result<(), StateWriteFailure> {
        self.event(Event::StreakWrite(lane.into(), value));
        self.0.borrow_mut().streaks.insert(lane.into(), value);
        Ok(())
    }
}

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
    fn alert(&self, target: AlertTarget<'_>, _summary: &str) -> AlertOutcome {
        self.event(Event::Alert(match target {
            AlertTarget::Run => "run".into(),
            AlertTarget::Lane(name) => name.into(),
        }));
        if self.0.borrow().fail_alerts {
            AlertOutcome::Failed("fixture transport failure".into())
        } else {
            AlertOutcome::Delivered
        }
    }
    fn record(&self, record: RunRecord<'_>) -> RecordOutcome {
        self.event(Event::Record(
            record.failures,
            record.deferred,
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
            Notice::StreakWriteFailed { .. } => panic!("fixture streak writes succeed"),
        };
        self.event(Event::Notice(text));
    }
}
