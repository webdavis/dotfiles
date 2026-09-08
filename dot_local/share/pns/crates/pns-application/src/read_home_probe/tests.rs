use super::*;
use pns_domain::home::{Client, HomePresence};
use std::cell::RefCell;

#[derive(Clone, Copy)]
enum Listing {
    Stale,
    Resolved,
    Away,
    Unknown,
}

struct Recorder {
    listing: Listing,
    memory: RefCell<Option<String>>,
    calls: RefCell<Vec<&'static str>>,
    cards: RefCell<Vec<EventArgs>>,
}

impl Recorder {
    fn new(listing: Listing, memory: Option<String>) -> Self {
        Self {
            listing,
            memory: RefCell::new(memory),
            calls: RefCell::new(Vec::new()),
            cards: RefCell::new(Vec::new()),
        }
    }

    fn run(&self) -> HomeReading {
        ReadHomeProbe {
            router: self,
            memory: self,
            notifier: self,
        }
        .run(&device(), "router-warning".to_string(), |reading, alert| {
            self.calls.borrow_mut().push("report");
            assert_eq!(
                alert.is_some(),
                matches!(reading.presence, HomePresence::Home { .. })
                    && matches!(self.listing, Listing::Stale)
                    && self.memory.borrow().is_none()
            );
        })
    }
}

impl Router for Recorder {
    fn clients(&self) -> Option<Vec<Client>> {
        self.calls.borrow_mut().push("read");
        match self.listing {
            Listing::Unknown => None,
            Listing::Away => Some(Vec::new()),
            Listing::Stale | Listing::Resolved => Some(vec![Client {
                name: Some(
                    if matches!(self.listing, Listing::Stale) {
                        "new-name"
                    } else {
                        "phone"
                    }
                    .to_string(),
                ),
                mac: Some("aa:bb:cc:dd:ee:ff".to_string()),
                ipv4: None,
            }]),
        }
    }
}

impl StalenessMemory for Recorder {
    fn remembered(&self) -> Option<String> {
        self.calls.borrow_mut().push("remembered");
        self.memory.borrow().clone()
    }

    fn remember(&self, episode: Option<&str>) {
        self.calls.borrow_mut().push("remember");
        *self.memory.borrow_mut() = episode.map(str::to_string);
    }
}

impl RaiseNotification for Recorder {
    fn raise(&self, event: &EventArgs) {
        self.calls.borrow_mut().push("notify");
        self.cards.borrow_mut().push(EventArgs {
            agent: event.agent.clone(),
            state: event.state.clone(),
            detail: event.detail.clone(),
            channel: event.channel.clone(),
            ..EventArgs::default()
        });
    }
}

fn device() -> DeviceIdentity {
    DeviceIdentity::new(
        Some("phone".to_string()),
        None,
        Some("aa:bb:cc:dd:ee:ff".to_string()),
    )
    .expect("valid fixture identity")
}

#[test]
fn a_new_home_disagreement_is_reported_then_notified_then_remembered() {
    let recorder = Recorder::new(Listing::Stale, None);
    let reading = recorder.run();
    assert_eq!(
        *recorder.calls.borrow(),
        ["read", "remembered", "report", "notify", "remember"]
    );
    let stale = home::stale_identifiers(&reading).expect("the matched MAC has a new name");
    assert_eq!(
        recorder.memory.borrow().as_deref(),
        Some(home::episode_id(&stale).as_str())
    );
    let cards = recorder.cards.borrow();
    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].agent, "pns");
    assert_eq!(cards[0].state, "stale");
    assert_eq!(cards[0].detail, home::stale_warning(&stale));
    assert_eq!(cards[0].channel, "router-warning");
}

#[test]
fn an_already_reported_home_disagreement_does_not_raise_another_card() {
    let first = Recorder::new(Listing::Stale, None);
    first.run();
    let next = Recorder::new(Listing::Stale, first.memory.borrow().clone());
    next.run();
    assert_eq!(
        *next.calls.borrow(),
        ["read", "remembered", "report", "remember"]
    );
    assert!(next.cards.borrow().is_empty());
}

#[test]
fn away_and_unknown_readings_preserve_the_previous_staleness_episode() {
    for listing in [Listing::Away, Listing::Unknown] {
        let recorder = Recorder::new(listing, Some("previous episode".to_string()));
        let reading = recorder.run();
        assert_eq!(*recorder.calls.borrow(), ["read", "remembered", "report"]);
        assert_eq!(
            recorder.memory.borrow().as_deref(),
            Some("previous episode")
        );
        assert!(recorder.cards.borrow().is_empty());
        assert_eq!(
            reading.presence,
            if matches!(listing, Listing::Away) {
                HomePresence::NotHome
            } else {
                HomePresence::Unknown
            }
        );
    }
}

#[test]
fn a_home_reading_that_resolved_the_disagreement_clears_its_memory_after_reporting() {
    let recorder = Recorder::new(Listing::Resolved, Some("previous episode".to_string()));
    recorder.run();
    assert_eq!(
        *recorder.calls.borrow(),
        ["read", "remembered", "report", "remember"]
    );
    assert!(recorder.memory.borrow().is_none());
    assert!(recorder.cards.borrow().is_empty());
}
