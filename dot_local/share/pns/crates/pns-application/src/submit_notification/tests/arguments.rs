use super::{Recorder, delivered_decision, event, ports, submission};
use pns_domain::Overrides;

#[test]
fn replay_and_edge_claim_receive_this_events_exact_legs_and_clock() {
    use pns_domain::routing::{Leg, ReportMode};

    for now in [None, Some(41), Some(u64::MAX)] {
        let (event, overrides) = (event(), Overrides::default());
        let mut decision = delivered_decision();
        decision.inputs.now_secs = now;
        decision.legs = vec![
            Leg {
                name: "mobile",
                mode: ReportMode::Silent,
                decorative: true,
            },
            Leg {
                name: "hermes",
                mode: ReportMode::ReportOutcome,
                decorative: false,
            },
        ];
        let recorder = Recorder::default();
        ports(&recorder).record(&submission(&event, &decision, &overrides));
        assert_eq!(*recorder.replays.borrow(), [(now, decision.legs)]);
        assert_eq!(*recorder.claims.borrow(), [(now, false)]);
    }
}
