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
/// object. The pane is the SANITIZED one.
#[derive(Debug, Default, PartialEq)]
pub struct Event {
    pub agent: String,
    pub state: String,
    pub project: String,
    pub branch: String,
    pub detail: String,
    pub title: String,
    pub message: String,
    pub preview: String,
    pub pane: String,
}

impl Event {
    /// The event as the JSON object the channel contract specifies, for an
    /// executable channel reading one line on stdin. The delivery mode is the
    /// one field that is per-LEG rather than per-event, so it arrives as an
    /// argument instead of living on the struct.
    pub fn to_json(&self, mode: ReportMode) -> String {
        serde_json::json!({
            "agent": self.agent,
            "state": self.state,
            "project": self.project,
            "branch": self.branch,
            "detail": self.detail,
            "title": self.title,
            "message": self.message,
            "preview": self.preview,
            "pane": self.pane,
            "mode": mode.as_str(),
        })
        .to_string()
    }
}

/// What one delivery has to say for itself.
///
/// A channel decides HOW to deliver and whether it can, never WHETHER it
/// should fire, and it must never fail the caller. Nothing here is an error
/// path: this exists so the one caller decides whether a line reaches the
/// operator, instead of each channel deciding for itself and only one of them
/// having an opinion.
///
/// THE VERDICT IS THE VARIANT, never a word inside the sentence. A caller that
/// had to find "FAILED" in the text to learn whether a destination received
/// anything would be a predicate keyed on English, and one of those has already
/// cost this repo a defect.
///
/// THE SENTENCE CARRIES NO `pns: ` PREFIX. It is added by the one place that
/// prints, so a caller that labels a line with the plugin's name does not have
/// to unpick a prefix out of the middle of its own.
#[derive(Debug, PartialEq)]
pub enum Delivery {
    /// Nothing worth saying, which is almost always the case.
    Silent,
    /// It arrived, and this is what the destination said about it.
    Delivered(String),
    /// It did not, and this is what the destination said about that.
    Failed(String),
}

impl Delivery {
    /// The line to print for this leg, or None. REPORT MODE IS THE CALLER'S
    /// to know: a channel says what happened, never whether anyone hears it.
    /// BOTH verdicts are printed on a reporting leg, because a failure is
    /// exactly the outcome the log path exists to make visible.
    pub fn line_for(self, mode: ReportMode) -> Option<String> {
        match self {
            Delivery::Delivered(line) | Delivery::Failed(line)
                if mode == ReportMode::ReportOutcome =>
            {
                Some(line)
            }
            _ => None,
        }
    }
}

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
            serde_json::from_str(&event.to_json(ReportMode::Silent)).unwrap();
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
    }

    #[test]
    fn the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways() {
        let event = Event {
            title: "t".to_string(),
            ..Event::default()
        };
        let sync: serde_json::Value =
            serde_json::from_str(&event.to_json(ReportMode::ReportOutcome)).unwrap();
        assert_eq!(sync["mode"], "sync");
        let asynchronous: serde_json::Value =
            serde_json::from_str(&event.to_json(ReportMode::Silent)).unwrap();
        assert_eq!(asynchronous["mode"], "async");
    }
}
