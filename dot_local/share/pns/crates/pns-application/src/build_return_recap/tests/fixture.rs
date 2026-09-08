use super::super::*;
use crate::Fetched;
use pns_domain::{EventArgs, missed::Entry};
use std::cell::{Cell, RefCell};

pub(super) struct World {
    pub log: RefCell<Vec<String>>,
    pub left: Cell<u64>,
    pub entries: Vec<Entry>,
    pub available: bool,
}
impl World {
    pub fn one() -> Self {
        Self {
            log: RefCell::new(Vec::new()),
            left: Cell::new(0),
            available: true,
            entries: vec![Entry {
                at: None,
                agent: "agent".into(),
                state: "done".into(),
                detail: "finished".into(),
                ..Entry::default()
            }],
        }
    }
    pub fn build(&self, recap: &Recap) -> String {
        BuildReturnRecap {
            activity: self,
            merges: self,
            notes: self,
            summarizer: self,
        }
        .run(
            recap,
            100,
            200,
            |at| super::super::recap_wall_clock(at, |_| None),
            |budget| {
                self.log.borrow_mut().push("budget".into());
                self.left.set(budget.as_secs());
                || Duration::from_secs(self.left.get())
            },
        )
    }
}
impl ActivityRing for World {
    fn record(&self, _: &EventArgs, _: Option<u64>) {
        panic!("a recap must not record");
    }
    fn entries_between(&self, since: u64, until: u64) -> Vec<Entry> {
        assert_eq!((since, until), (100, 200));
        self.log.borrow_mut().push("activity".into());
        self.entries.clone()
    }
}
impl MergedPullRequestSource for World {
    fn merged(&self, repos: &[String], since: u64, until: u64) -> Option<Fetched> {
        assert_eq!(repos, &["owner/repo"]);
        assert_eq!((since, until), (100, 200));
        self.log.borrow_mut().push("merges".into());
        self.available.then(|| Fetched {
            sources: vec![pns_domain::recap::external::merged(
                42,
                "merge title",
                "merge body",
            )],
            truncated: false,
        })
    }
}
impl ReviewNoteSource for World {
    fn notes(&self, pattern: &str, since: u64, until: u64) -> Option<Fetched> {
        assert_eq!(pattern, "notes/*.md");
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
    fn summarize(&self, argv: &[String], deadline: Duration, prompt: &str) -> Option<Vec<String>> {
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
        repos: vec!["owner/repo".into()],
        review_notes: Some("notes/*.md".into()),
        summarizer: Some(vec!["summary".into(), "--plain".into()]),
        summarizer_deadline_secs: 6,
        ..Recap::default()
    }
}
