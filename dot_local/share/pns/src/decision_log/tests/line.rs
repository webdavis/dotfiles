use super::super::{Record, line};
use super::fixtures::{decision, event, inputs};
use crate::channels::Delivery;
use crate::engine::{Decision, GateInputs, Overrides};
use crate::routing::{Leg, ReportMode};
use crate::surface::DeliveryPlan;

#[test]
fn a_line_names_the_event_and_every_gate_input_behind_one_epoch_second() {
    // EVERY VALUE IS A NUMBER, A BOOLEAN OR AN ENUM NAME, so the only
    // reader this file has can print it without interpreting it, and a
    // reading nobody could take stays absent instead of becoming a zero.
    let plain = decision(inputs());
    let overrides = Overrides {
        skip_phone: true,
        ..Overrides::default()
    };
    assert_eq!(
        line(&Record {
            event: &event(),
            decision: &plain,
            overrides: &overrides,
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        }),
        "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
             session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=no \
             fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
             pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
             idle_invalid=no desk_invalid=no phone_invalid=no \
             plan=banner:no,card:no,pulse:no legs=none"
    );

    // AN UNREAD LOCK IS ITS OWN ROW, byte for byte. A `locked=no` here
    // would be the line claiming the display was awake on a reading the
    // decision never took, which is the one thing `tri` exists to stop.
    let unread_lock = decision(GateInputs {
        screen_locked: None,
        ..inputs()
    });
    assert_eq!(
        line(&Record {
            event: &event(),
            decision: &unread_lock,
            overrides: &overrides,
            legs: &[],
            nag: false,
            permission_mode: "",
            agent_id: "",
            tool_name: "",
        }),
        "1756500000 claude/blocked mode=none agent=none tool=none surface=Mobile visibility=Hidden \
             session_visibility=Visible desk_age=none phone_age=12 tap_age=none locked=none \
             fresh_window=120 long_running=no nag=no local_only=no remote_only=no pane=present \
             pane_dropped=no watch_card=no muted=no focus=no skip_phone=yes force_phone=no \
             idle_invalid=no desk_invalid=no phone_invalid=no \
             plan=banner:no,card:no,pulse:no legs=none"
    );
}

#[test]
fn a_line_with_no_readable_clock_leads_with_a_dash_rather_than_epoch_zero() {
    // A RECOGNIZED VALUE, so the reader can tell it from a line it could
    // not parse. Epoch zero would parse cleanly and render as 56 years
    // ago, which is a claim nobody measured.
    let decision = decision(GateInputs {
        now_secs: None,
        ..inputs()
    });
    let recorded = line(&Record {
        event: &event(),
        decision: &decision,
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.starts_with("- claude/blocked "),
        "got {recorded:?}"
    );
}

#[test]
fn a_line_carries_the_arbitrated_plan_and_each_legs_verdict() {
    // WITHOUT THE LEGS the log says pns decided to card the operator while
    // their question is why no card appeared. THE VERDICT IS THE VARIANT
    // NAME, never the channel's sentence, which can carry a status code or
    // a URL.
    let carded = decision(inputs());
    let carded = Decision {
        plan: DeliveryPlan {
            banner: false,
            phone_card: true,
            pulse: false,
        },
        ..carded
    };
    // THE DECORATION FLAG IS THE ROSTER'S OWN: the phone and the banner
    // show the operator something, the durable log and an unknown channel
    // do not. Nothing in a ring line reads it, which is exactly why it is
    // stated honestly here rather than defaulted.
    let legs = [
        (
            Leg {
                name: "mobile",
                mode: ReportMode::Silent,
                decorative: true,
            },
            Delivery::Failed("the gateway answered 502 at https://example.invalid".to_string()),
        ),
        (
            Leg {
                name: "hermes",
                mode: ReportMode::Silent,
                decorative: false,
            },
            Delivery::Delivered("posted".to_string()),
        ),
        (
            Leg {
                name: "macos-banner",
                mode: ReportMode::Silent,
                decorative: true,
            },
            Delivery::Silent,
        ),
        (
            Leg {
                name: "kitchen",
                mode: ReportMode::Silent,
                decorative: false,
            },
            Delivery::Unlaunched("no such channel".to_string()),
        ),
    ];
    let recorded = line(&Record {
        event: &event(),
        decision: &carded,
        overrides: &Overrides::default(),
        legs: &legs,
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.ends_with(
            " plan=banner:no,card:yes,pulse:no \
                 legs=mobile:failed,hermes:delivered,macos-banner:silent,kitchen:unlaunched"
        ),
        "got {recorded}"
    );
    assert!(
        !recorded.contains("502"),
        "the sentence stays out: {recorded}"
    );
    assert!(
        !recorded.contains("example.invalid"),
        "and its URL with it: {recorded}"
    );

    // A PLAN THAT REACHED NO CHANNEL still records, and says so.
    let recorded = line(&Record {
        event: &event(),
        decision: &decision(inputs()),
        overrides: &Overrides::default(),
        legs: &[],
        nag: false,
        permission_mode: "",
        agent_id: "",
        tool_name: "",
    });
    assert!(
        recorded.ends_with(" plan=banner:no,card:no,pulse:no legs=none"),
        "got {recorded}"
    );
}
