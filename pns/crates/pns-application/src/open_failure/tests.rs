use super::*;
use std::cell::RefCell;

/// Answers success or failure per call, in order, and records every argv.
struct Runner {
    answers: RefCell<Vec<bool>>,
    seen: RefCell<Vec<Vec<String>>>,
}

impl Runner {
    fn new(answers: &[bool]) -> Self {
        Self {
            answers: RefCell::new(answers.to_vec()),
            seen: RefCell::new(Vec::new()),
        }
    }
}

impl CommandRunner for Runner {
    fn run(&self, program: &str, args: &[&str]) -> Option<String> {
        let mut argv = vec![program.to_string()];
        argv.extend(args.iter().map(|arg| arg.to_string()));
        self.seen.borrow_mut().push(argv);
        let mut answers = self.answers.borrow_mut();
        if answers.is_empty() {
            None
        } else if answers.remove(0) {
            Some(String::new())
        } else {
            None
        }
    }
}

const HERDR: &str = "/bin/herdr";
const PNS: &str = "/bin/pns";

/// A click that works runs ONE command. A second attempt after a success would
/// open the record twice.
#[test]
fn a_click_that_works_runs_one_command_and_stops() {
    let runner = Runner::new(&[true]);
    let outcome = open_failure(&runner, &ClickView::Herdr, 47, HERDR, PNS);
    assert!(outcome.opened);
    assert!(!outcome.exhausted);
    assert_eq!(outcome.tried.len(), 1);
    assert_eq!(runner.seen.borrow().len(), 1);
}

/// The fallback is `window` whatever was configured, because it needs no
/// session, no config and no operator string: it is the one attempt that cannot
/// fail for the reason the first one did.
#[test]
fn a_click_that_fails_falls_back_to_a_plain_window() {
    let runner = Runner::new(&[false, true]);
    let outcome = open_failure(&runner, &ClickView::Herdr, 47, HERDR, PNS);
    assert!(outcome.opened);
    assert!(!outcome.exhausted);
    assert_eq!(outcome.tried.len(), 2);
    assert_eq!(outcome.tried[0][0], HERDR);
    assert_eq!(outcome.tried[1][0], "/usr/bin/open");
}

/// A configured command that does not exist is the case the design names: it
/// produces the fallback and, when that also fails, one exhausted outcome.
#[test]
fn a_command_that_does_not_exist_falls_back_once_and_then_gives_up() {
    let runner = Runner::new(&[false, false]);
    let outcome = open_failure(
        &runner,
        &ClickView::Command("/nowhere/viewer {id}".into()),
        47,
        HERDR,
        PNS,
    );
    assert!(!outcome.opened);
    assert!(outcome.exhausted);
    assert_eq!(outcome.tried.len(), 2, "one fallback, never two");
    assert_eq!(outcome.tried[0], ["/nowhere/viewer", "47"]);
}

/// A view that IS the fallback does not get a second identical try: spending
/// another second reaching the same place is not a fallback.
#[test]
fn the_window_view_is_never_tried_twice() {
    let runner = Runner::new(&[false, true]);
    let outcome = open_failure(&runner, &ClickView::Window, 47, HERDR, PNS);
    assert!(!outcome.opened);
    assert!(outcome.exhausted);
    assert_eq!(outcome.tried.len(), 1);
    assert_eq!(runner.seen.borrow().len(), 1);
}

/// A command view with nothing in it runs nothing, and still gets the fallback:
/// the operator's unfinished config must not cost them the record.
#[test]
fn an_empty_command_view_runs_nothing_and_still_falls_back() {
    let runner = Runner::new(&[true]);
    let outcome = open_failure(&runner, &ClickView::Command(String::new()), 47, HERDR, PNS);
    assert!(outcome.opened);
    assert_eq!(outcome.tried.len(), 1);
    assert_eq!(outcome.tried[0][0], "/usr/bin/open");
}

/// The log line names every command tried, so the operator can run one by hand
/// and see the real error rather than pns's summary of it.
#[test]
fn the_log_line_names_every_command_it_tried() {
    let runner = Runner::new(&[false, false]);
    let outcome = open_failure(&runner, &ClickView::Herdr, 47, HERDR, PNS);
    let line = click_failure_line(47, &outcome);
    assert!(line.contains("failure 47"), "{line}");
    assert!(line.contains(HERDR), "{line}");
    assert!(line.contains("/usr/bin/open"), "{line}");
    assert!(line.contains("; then "), "{line}");
}

/// The last-resort banner names the failure and a command that needs no view,
/// because a view is exactly what just failed twice.
#[test]
fn the_last_resort_banner_offers_a_command_rather_than_another_view() {
    let (title, message) = click_banner(47);
    assert!(title.contains("47"), "{title}");
    assert_eq!(message, "run `pns failures 47` in a terminal");
}
