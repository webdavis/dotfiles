use super::*;
use pns_domain::Answers;
use std::cell::RefCell;
use std::collections::VecDeque;

struct World {
    trace: RefCell<Vec<String>>,
    answers: RefCell<VecDeque<Result<String, String>>>,
    observed: RefCell<Option<Answers>>,
    output: RefCell<Vec<String>>,
    terminal: bool,
    check: Result<(), String>,
    validate: Result<(), String>,
    publish: Result<Option<String>, String>,
}
impl World {
    fn new(answers: &[&str]) -> Self {
        Self {
            trace: RefCell::new(Vec::new()),
            answers: RefCell::new(answers.iter().map(|s| Ok(s.to_string())).collect()),
            observed: RefCell::new(None),
            output: RefCell::new(Vec::new()),
            terminal: true,
            check: Ok(()),
            validate: Ok(()),
            publish: Ok(None),
        }
    }
    fn run(&self, force: bool) -> (i32, Vec<String>) {
        let mut errors = Vec::new();
        let code = RunSetup {
            terminal: self,
            renderer: self,
            publisher: self,
        }
        .run(force, |line| errors.push(line.to_string()));
        (code, errors)
    }
    fn answer(&self, kind: &str, question: &str) -> Result<String, String> {
        self.trace.borrow_mut().push(format!("{kind}: {question}"));
        self.answers
            .borrow_mut()
            .pop_front()
            .expect("question was scripted")
    }
}
impl Terminal for World {
    fn is_terminal(&self) -> bool {
        self.trace.borrow_mut().push("terminal".into());
        self.terminal
    }
    fn say(&self, line: &str) {
        self.output.borrow_mut().push(line.into());
    }
    fn ask(&self, question: &str) -> Result<String, String> {
        self.answer("plain", question)
    }
    fn ask_hidden(&self, question: &str) -> Result<String, String> {
        self.answer("secret", question)
    }
}
impl ConfigRenderer for World {
    fn compose(&self, answers: &Answers) -> String {
        self.trace.borrow_mut().push("compose".into());
        *self.observed.borrow_mut() = Some(Answers {
            mobile_token: answers.mobile_token.clone(),
            hermes_key: answers.hermes_key.clone(),
            hue_bridge: answers.hue_bridge.clone(),
            hue_key: answers.hue_key.clone(),
            hue_rooms: answers.hue_rooms.clone(),
            router_type: answers.router_type.clone(),
            router_url: answers.router_url.clone(),
            router_api_key: answers.router_api_key.clone(),
            router_device_hostname: answers.router_device_hostname.clone(),
            focus_modes: answers.focus_modes.clone(),
            nag: answers.nag,
        });
        "exact composed bytes".into()
    }
    fn validate(&self, composed: &str) -> Result<(), String> {
        assert_eq!(composed, "exact composed bytes");
        self.trace.borrow_mut().push("validate".into());
        self.validate.clone()
    }
}
impl ConfigPublisher for World {
    fn path(&self) -> String {
        "private config".into()
    }
    fn check(&self, force: bool) -> Result<(), String> {
        self.trace.borrow_mut().push(format!("check {force}"));
        self.check.clone()
    }
    fn publish(&self, composed: &str, force: bool) -> Result<Option<String>, String> {
        assert_eq!(composed, "exact composed bytes");
        self.trace.borrow_mut().push(format!("publish {force}"));
        self.publish.clone()
    }
}
const DECLINED: [&str; 6] = ["", "", "", "", "", ""];

mod lifecycle;
mod questions;
