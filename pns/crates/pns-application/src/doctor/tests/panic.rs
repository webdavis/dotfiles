use super::*;
use crate::{DoctorBridge, LampBridge, LampWrite, deliver_guarded, doctor_lamps, doctor_pulse};
use pns_domain::lamps::{Inventory, config::Lights};

#[test]
fn panicking_delivery_is_one_failed_leg_and_the_next_leg_still_runs() {
    let outcomes = ["alpha", "beta"].map(|name| {
        leg(
            name,
            deliver_guarded(name, || {
                if name == "alpha" {
                    panic!("owned panic fixture")
                }
                Delivery::Silent
            }),
        )
    });
    assert_eq!(
        outcomes[0].1,
        Delivery::Failed("the alpha channel PANICKED; nothing was sent".into())
    );
    assert_eq!(outcomes[1].1, Delivery::Silent);
    let (code, lines) = report(
        &History::default(),
        outcomes.to_vec(),
        Outcome::Signalled(1),
        Pairing::NoAnswer,
    );
    assert_eq!(code, 1);
    assert!(lines.last().unwrap().contains("missed"));
}

#[test]
fn panicking_pulse_reports_no_room_and_does_not_end_the_doctor() {
    let pulse = doctor_pulse(true, || panic!("owned pulse fixture"));
    assert_eq!(
        pulse,
        Outcome::Failed("the pulse PANICKED; no room was signalled".into())
    );
    let (code, lines) = report(&History::default(), sent(), pulse, Pairing::NoAnswer);
    assert_eq!(code, 1);
    assert!(lines.last().unwrap().contains("missed"));
    assert_eq!(doctor_pulse(true, || 2), Outcome::Signalled(2));
    assert!(matches!(
        doctor_pulse(false, || panic!("unconfigured pulse must not run")),
        Outcome::Failed(_)
    ));
}

struct Bridge(bool);
impl LampBridge for Bridge {
    fn inventory(&self) -> Option<Inventory> {
        assert!(!self.0, "owned inventory panic fixture");
        Some(Inventory::default())
    }
    fn write(&self, _: &str, _: &LampWrite) {
        panic!("doctor inventory must never write lamps")
    }
}

#[test]
fn panicking_lamp_listing_is_unreachable_while_healthy_listing_resolves() {
    let lights = Lights::default();
    assert!(matches!(
        doctor_lamps(Some(&lights), || DoctorBridge::Ready(Bridge(true))),
        LightsReport::Unreachable
    ));
    assert!(matches!(
        doctor_lamps(Some(&lights), || DoctorBridge::Ready(Bridge(false))),
        LightsReport::Resolved(_)
    ));
    assert!(matches!(
        doctor_lamps::<Bridge>(None, || panic!("off must not build bridge")),
        LightsReport::Off
    ));
}
