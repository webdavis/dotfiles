//! Native channel plugins: the compiled-in delivery half of a registration.
//!
//! DISPATCH PRECEDENCE, decided here once: with `PNS_CHANNELS_DIR` explicitly
//! set, executables win for every name, which is the test seam and the
//! operator's escape hatch. With it unset, a native plugin wins and the
//! executable fallback serves only names that have no native implementation
//! yet. That rule is what lets each channel slice go native without touching
//! the dispatch bats: the stubs keep winning wherever the suite points the
//! engine at a stub directory.

pub mod banner;
pub mod hermes;
pub mod hue;
pub mod moshi;

use crate::routing::ReportMode;

/// One rendered event, the structured form of the channel contract's JSON
/// object. THE VALUE MOVED to `pns-domain`, where the ports that hand it to a
/// destination are declared; the wire format below stayed, because it is JSON.
pub use pns_domain::notification::Event;

/// The event as the JSON object the channel contract specifies, for an
/// executable channel reading one line on stdin. The delivery mode is the
/// one field that is per-LEG rather than per-event, so it arrives as an
/// argument instead of living on the struct.
///
/// A FREE FUNCTION RATHER THAN A METHOD, because an inherent impl may only be
/// written in the crate that defines the type, and `Event` is the domain's now.
pub fn event_json(event: &Event, mode: ReportMode) -> String {
    serde_json::json!({
    "agent": event.agent,
    "state": event.state,
    "project": event.project,
    "branch": event.branch,
    "detail": event.detail,
    "title": event.title,
    "message": event.message,
    "preview": event.preview,
    "pane": event.pane,
    "mode": mode.as_str(),
    })
    .to_string()
}

/// What one delivery has to say for itself.
///
/// THE VOCABULARY IS THE ROUTING'S, in `pns-domain` beside the `Leg` it
/// answers for and the `ReportMode` that says whether anyone reads it, and it
/// is named here for every destination that produces one.
pub use pns_domain::routing::Delivery;

/// True when native plugins take precedence for dispatch: only when the
/// channels directory was NOT explicitly overridden.
pub fn native_first(channels_dir_overridden: bool) -> bool {
    !channels_dir_overridden
}

#[cfg(test)]
mod tests {
    use super::{Delivery, Event};
    use crate::routing::ReportMode;

    #[test]
    fn the_event_is_the_channel_contracts_json_object() {
        let event = Event {
            agent: "claude".to_string(),
            state: "done".to_string(),
            project: "dotfiles".to_string(),
            branch: "main".to_string(),
            detail: "a \"quoted\" detail".to_string(),
            title: "claude done: dotfiles".to_string(),
            message: "main: a detail".to_string(),
            preview: "a preview".to_string(),
            pane: "wW:p21".to_string(),
        };
        let parsed: serde_json::Value =
            serde_json::from_str(&super::event_json(&event, ReportMode::Silent)).unwrap();
        assert_eq!(parsed["agent"], "claude");
        assert_eq!(parsed["detail"], "a \"quoted\" detail");
        assert_eq!(parsed["pane"], "wW:p21");
        assert_eq!(parsed["mode"], "async");
        assert_eq!(parsed["title"], "claude done: dotfiles");
    }

    #[test]
    fn either_verdict_reaches_the_operator_on_a_reporting_leg_and_nothing_does_otherwise() {
        // The whole policy, in one place: an async leg says nothing however
        // much the channel had to say, and a silent channel says nothing
        // however the leg reports. The verdict never decides who hears it,
        // only the mode does, so a failure is as printable as a success.
        assert_eq!(
            Delivery::Delivered("posted HTTP 200".to_string()).line_for(ReportMode::ReportOutcome),
            Some("posted HTTP 200".to_string())
        );
        assert_eq!(
            Delivery::Failed("post FAILED HTTP 401".to_string())
                .line_for(ReportMode::ReportOutcome),
            Some("post FAILED HTTP 401".to_string())
        );
        assert_eq!(
            Delivery::Delivered("posted HTTP 200".to_string()).line_for(ReportMode::Silent),
            None
        );
        assert_eq!(
            Delivery::Failed("post FAILED HTTP 401".to_string()).line_for(ReportMode::Silent),
            None
        );
        assert_eq!(Delivery::Silent.line_for(ReportMode::ReportOutcome), None);
        assert_eq!(Delivery::Silent.line_for(ReportMode::Silent), None);
        // AND A CHANNEL THAT NEVER LAUNCHED IS SWALLOWED IN BOTH MODES. It is
        // the uninstalled-channel case, which has never been news on the
        // notification path; the hand-run check is the one caller that reads
        // it, and it reads the variant rather than a printed line.
        for mode in [ReportMode::ReportOutcome, ReportMode::Silent] {
            assert_eq!(
                Delivery::Unlaunched("could not launch the channel".to_string()).line_for(mode),
                None,
                "mode: {mode:?}"
            );
        }
    }

    #[test]
    fn the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways() {
        let event = Event {
            title: "t".to_string(),
            ..Event::default()
        };
        let sync: serde_json::Value =
            serde_json::from_str(&super::event_json(&event, ReportMode::ReportOutcome)).unwrap();
        assert_eq!(sync["mode"], "sync");
        let asynchronous: serde_json::Value =
            serde_json::from_str(&super::event_json(&event, ReportMode::Silent)).unwrap();
        assert_eq!(asynchronous["mode"], "async");
    }
}
