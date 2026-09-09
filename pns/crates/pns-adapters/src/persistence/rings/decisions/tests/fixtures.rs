use pns_domain::EventArgs;
use pns_domain::surface::{DeliveryPlan, Surface, Visibility};
use pns_domain::{Decision, GateInputs};

/// The readings behind one decision, distinct in every field so a swap
/// between two of them cannot pass.
pub(super) fn inputs() -> GateInputs {
    GateInputs {
        now_secs: Some(1_756_500_000),
        desk_input_age: None,
        phone_input_age: Some(12),
        marker_age: None,
        screen_locked: Some(false),
        desk_fresh_secs: Some(120),
        surface: Surface::Mobile,
        session_visibility: Visibility::Visible,
        visibility: Visibility::Hidden,
        long_running: false,
        mobile_watch_card: false,
        scope: pns_domain::DeliveryScope::Automatic,
        pane_present: true,
    }
}

pub(super) fn decision(inputs: GateInputs) -> Decision {
    Decision {
        legs: Vec::new(),
        plan: DeliveryPlan {
            banner: false,
            phone_card: false,
            pulse: false,
        },
        pane_dropped: false,
        inputs,
    }
}

pub(super) fn event() -> EventArgs {
    EventArgs {
        agent: "claude".to_string(),
        state: "blocked".to_string(),
        ..EventArgs::default()
    }
}
