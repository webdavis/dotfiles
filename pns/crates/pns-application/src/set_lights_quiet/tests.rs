use super::SetLightsQuiet;
use crate::LampMutes;
use pns_domain::lights::mute::{Muted, QuietCommand};
use std::cell::RefCell;

#[derive(Default)]
struct Mutes {
    trace: RefCell<Vec<String>>,
    entries: Vec<Muted>,
    complaints: Vec<String>,
    fail: bool,
}
impl LampMutes for Mutes {
    fn read(&self) -> (Vec<Muted>, Vec<String>) {
        self.trace.borrow_mut().push("read".into());
        (self.entries.clone(), self.complaints.clone())
    }
    fn write(&self, entries: &[Muted]) -> Result<(), String> {
        self.trace.borrow_mut().push(format!("write:{entries:?}"));
        if self.fail {
            Err("fixture".into())
        } else {
            Ok(())
        }
    }
}

#[test]
fn a_lights_quiet_report_never_republishes_the_record_it_read() {
    let mutes = Mutes::default();
    let lines = SetLightsQuiet { mutes: &mutes }
        .run(&QuietCommand::Report, Some(100), |_| {
            panic!("unexpected warning")
        })
        .unwrap();
    assert_eq!(*mutes.trace.borrow(), ["read"]);
    assert!(!lines.is_empty());
}

#[test]
fn a_lights_mute_warns_before_replacing_unreadable_state() {
    let mutes = Mutes {
        complaints: vec!["unreadable".into()],
        ..Default::default()
    };
    let lines = SetLightsQuiet { mutes: &mutes }
        .run(
            &QuietCommand::Mute {
                place: "studio".into(),
                seconds: 60,
            },
            Some(100),
            |warning| mutes.trace.borrow_mut().push(warning.into()),
        )
        .unwrap();
    assert_eq!(&mutes.trace.borrow()[..2], ["read", "unreadable"]);
    assert!(mutes.trace.borrow()[2].contains("studio"));
    assert!(mutes.trace.borrow()[2].contains("160"));
    assert!(lines.iter().any(|line| line.contains("studio")));
}

#[test]
fn a_refused_lights_mute_or_unmute_returns_no_success_report() {
    for command in [
        QuietCommand::Mute {
            place: "studio".into(),
            seconds: 60,
        },
        QuietCommand::Unmute {
            place: "studio".into(),
        },
    ] {
        let mutes = Mutes {
            fail: true,
            ..Default::default()
        };
        assert_eq!(SetLightsQuiet { mutes: &mutes }.run(&command, Some(100), |_| panic!("warning")),
            Err("pns: state error (lights-quiet could not be written: fixture); the mute was not set".into()));
        assert_eq!(mutes.trace.borrow().len(), 2);
    }
}

#[test]
fn a_missing_clock_refuses_a_new_lights_mute_before_publication() {
    let mutes = Mutes::default();
    assert_eq!(
        SetLightsQuiet { mutes: &mutes }.run(
            &QuietCommand::Mute {
                place: "studio".into(),
                seconds: 60
            },
            None,
            |_| panic!("warning")
        ),
        Err("pns: state error (the clock cannot be read); the mute was not set".into())
    );
    assert_eq!(*mutes.trace.borrow(), ["read"]);
}
