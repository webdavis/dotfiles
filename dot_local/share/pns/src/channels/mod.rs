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

use crate::routing::Mode;

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
    pub fn to_json(&self, mode: Mode) -> String {
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

/// A native channel: it decides HOW to deliver and whether it can, never
/// WHETHER it should fire, and it must never fail the caller.
pub trait Channel {
    fn deliver(&self, event: &Event, mode: Mode);
}

/// True when native plugins take precedence for dispatch: only when the
/// channels directory was NOT explicitly overridden.
pub fn native_first(channels_dir_overridden: bool) -> bool {
    !channels_dir_overridden
}

#[cfg(test)]
mod tests {
    use super::Event;
    use crate::routing::Mode;

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
        let parsed: serde_json::Value = serde_json::from_str(&event.to_json(Mode::Async)).unwrap();
        assert_eq!(parsed["agent"], "claude");
        assert_eq!(parsed["detail"], "a \"quoted\" detail");
        assert_eq!(parsed["pane"], "wW:p21");
        assert_eq!(parsed["mode"], "async");
        assert_eq!(parsed["title"], "claude done: dotfiles");
    }

    #[test]
    fn the_mode_is_the_only_per_leg_field_so_one_event_serializes_both_ways() {
        let event = Event {
            title: "t".to_string(),
            ..Event::default()
        };
        let sync: serde_json::Value = serde_json::from_str(&event.to_json(Mode::Sync)).unwrap();
        assert_eq!(sync["mode"], "sync");
        let asynchronous: serde_json::Value =
            serde_json::from_str(&event.to_json(Mode::Async)).unwrap();
        assert_eq!(asynchronous["mode"], "async");
    }
}
