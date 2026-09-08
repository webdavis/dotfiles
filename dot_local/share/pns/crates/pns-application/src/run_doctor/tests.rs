use super::*;
use pns_domain::doctor::{CheckKind, Pairing};
use std::cell::RefCell;

struct History {
    decisions: Result<Option<String>, String>,
    journal: Result<Option<String>, String>,
    reads: RefCell<Vec<&'static str>>,
    imports: Result<Vec<(String, String)>, String>,
}
impl Default for History {
    fn default() -> Self {
        Self {
            decisions: Ok(None),
            journal: Ok(None),
            reads: RefCell::new(Vec::new()),
            imports: Ok(Vec::new()),
        }
    }
}
impl DecisionRing for History {
    fn record(&self, _: &pns_domain::Record) {
        panic!("doctor must not record its own send")
    }
    fn read(&self) -> Result<Option<String>, String> {
        self.reads.borrow_mut().push("decisions");
        self.decisions.clone()
    }
}
impl Journal for History {
    fn journal(&self, _: &EventArgs, _: Option<u64>) {
        panic!("doctor must not journal its own send")
    }
    fn read(&self) -> Result<Option<String>, String> {
        self.reads.borrow_mut().push("journal");
        self.journal.clone()
    }
}
fn report(
    history: &History,
    delivered: Vec<(Leg, Delivery)>,
    pulse: Outcome,
    pairing: Pairing,
) -> (i32, Vec<String>) {
    let checks = [
        Check {
            plugin: "alpha",
            kind: CheckKind::Send,
        },
        Check {
            plugin: "beta",
            kind: CheckKind::Send,
        },
        Check {
            plugin: "hue",
            kind: CheckKind::Pulse,
        },
        Check {
            plugin: "room",
            kind: CheckKind::Presence,
        },
        Check {
            plugin: "off",
            kind: CheckKind::Skipped("configured off"),
        },
    ];
    let mut lines = Vec::new();
    let code =
        RunDoctor {
            checks: &checks,
            records: history,
            clock: &|| Some(100),
            replay_card: false,
            nag_after_secs: 0,
        }
        .run(
            DoctorActions {
                deliver: |legs: &[Leg], event: &EventArgs| {
                    assert_eq!(
                        legs.iter().map(|l| l.name).collect::<Vec<_>>(),
                        ["alpha", "beta"]
                    );
                    assert!(legs.iter().all(|l| !l.decorative
                        && l.mode == pns_domain::routing::ReportMode::ReportOutcome));
                    assert_eq!(
                        (&*event.agent, &*event.state, &*event.detail),
                        ("pns", "doctor", DOCTOR_DETAIL)
                    );
                    assert!(event.pane.is_empty());
                    delivered
                },
                pulse: || pulse.clone(),
                presence: || (PresenceStatus::Nowhere { poll_age_secs: 2 }, None),
                pairing: || PairingReport {
                    pairing,
                    server: Some("fixture server".into()),
                },
                focus: || "focus fixture".into(),
                daemon: || "daemon fixture".into(),
                lamps: || LightsReport::Off,
                imports: || {
                    history.imports.clone().map(|rows| {
                        rows.into_iter()
                            .map(|(record, reason)| ImportFailure { record, reason })
                            .collect()
                    })
                },
            },
            |line| lines.push(line.to_string()),
        );
    (code, lines)
}
fn leg(name: &'static str, delivery: Delivery) -> (Leg, Delivery) {
    (
        Leg {
            name,
            mode: pns_domain::routing::ReportMode::ReportOutcome,
            decorative: false,
        },
        delivery,
    )
}
fn sent() -> Vec<(Leg, Delivery)> {
    vec![
        leg("beta", Delivery::Silent),
        leg("alpha", Delivery::Delivered("alpha receipt".into())),
    ]
}

mod composition;
mod focus;
mod panic;

mod imports;
