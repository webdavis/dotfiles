use super::super::*;
use crate::Fetched;
use pns_domain::recap::activity::Event;
use pns_domain::recap::external::Sourcing;
use pns_domain::recap::sections::body;
use std::cell::{Cell, RefCell};

pub(super) struct World {
    pub log: RefCell<Vec<String>>,
    pub left: Cell<u64>,
    pub events: Vec<Event>,
    pub available: bool,
    pub dead_lettered: usize,
}
impl World {
    pub fn one() -> Self {
        Self {
            log: RefCell::new(Vec::new()),
            left: Cell::new(0),
            available: true,
            dead_lettered: 0,
            events: vec![Event {
                at: 150,
                agent: "agent".into(),
                state: "done".into(),
                session: "one".into(),
                session_title: "finished".into(),
                detail: "finished".into(),
                ..Event::default()
            }],
        }
    }
    /// The assembled recap, rendered the way a delivery would render it.
    pub fn build(&self, recap: &Recap) -> String {
        let assembled = self.assemble(recap, Vec::new());
        let externals = assembled.externals();
        let clock = |at: Option<u64>| super::super::recap_wall_clock(at, |_| None);
        body(&assembled.page(&externals, &clock))
    }
    pub fn assemble(&self, recap: &Recap, sections: Vec<String>) -> Assembled {
        BuildRecap {
            activity: self,
            commands: self,
            notes: self,
            summarizer: self,
            failures: self,
        }
        .assemble(
            &Request {
                recap,
                window: None,
                previous: false,
                since: 100,
                until: 200,
                sections,
                verbose: false,
                limit: None,
                windowed: true,
            },
            |at| super::super::recap_wall_clock(at, |_| None),
            |budget| {
                self.log.borrow_mut().push("budget".into());
                self.left.set(budget.as_secs());
                || Duration::from_secs(self.left.get())
            },
        )
    }
}
impl ActivityEvents for World {
    fn activity_between(&self, since: u64, until: u64) -> Vec<Event> {
        assert_eq!((since, until), (100, 200));
        self.log.borrow_mut().push("activity".into());
        self.events.clone()
    }
}
impl SourceCommands for World {
    fn run(&self, argv: &[String], since: Option<u64>, until: Option<u64>) -> Sourcing {
        self.log
            .borrow_mut()
            .push(format!("run({} {since:?} {until:?})", argv.join(" ")));
        match self.available {
            false => Sourcing::Unavailable,
            true => Sourcing::Read(
                vec![pns_domain::recap::external::printed("#42 merge title")],
                false,
            ),
        }
    }
}
impl crate::DeadLetteredLegs for World {
    fn dead_lettered(&self) -> usize {
        self.dead_lettered
    }
}
impl ReviewNoteSource for World {
    fn notes(&self, pattern: &str, since: u64, until: u64) -> Option<Fetched> {
        assert_eq!(pattern, "/notes/*.md");
        assert_eq!((since, until), (100, 200));
        self.log.borrow_mut().push("notes".into());
        self.available.then(|| Fetched {
            sources: vec![pns_domain::recap::external::noted(
                "finding.md",
                "note body",
            )],
            truncated: true,
        })
    }
}
impl Summarizer for World {
    fn summarize(
        &self,
        invocation: &pns_domain::recap::summarizer::Invocation,
        deadline: Duration,
        prompt: &str,
    ) -> Option<Vec<String>> {
        let argv = invocation.argv.as_slice();
        assert_eq!(argv, &["summary", "--plain"]);
        assert!(!prompt.is_empty());
        self.log
            .borrow_mut()
            .push(format!("summary({})", deadline.as_secs()));
        self.left.set(self.left.get().saturating_sub(4));
        (!deadline.is_zero()).then(|| vec!["selected line".into()])
    }
}

pub(super) fn configured() -> Recap {
    Recap {
        sources: pns_domain::recap::Sources {
            pull_requests: Some(vec!["gh".into()]),
            ..pns_domain::recap::Sources::default()
        },
        review_notes_glob: Some("/notes/*.md".into()),
        summarizer: pns_domain::recap::summarizer::Settings {
            command: vec!["summary".into(), "--plain".into()],
            deadline: std::time::Duration::from_secs(6),
            ..Default::default()
        },
        ..Recap::default()
    }
}
