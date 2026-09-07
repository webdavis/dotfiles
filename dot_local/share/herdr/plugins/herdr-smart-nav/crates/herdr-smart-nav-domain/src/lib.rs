pub fn direction_to_chord(direction: &str) -> Option<&'static str> {
    match direction {
        "left" => Some("ctrl+h"),
        "down" => Some("ctrl+j"),
        "up" => Some("ctrl+k"),
        "right" => Some("ctrl+l"),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
pub enum Action {
    SendKeys { pane: String, chord: String },
    Focus { pane: String, direction: String },
    FocusCurrent { direction: String },
}

pub fn decide(pane: Option<&str>, is_nvim: bool, direction: &str, chord: &str) -> Action {
    match pane {
        Some(p) if is_nvim => Action::SendKeys {
            pane: p.to_string(),
            chord: chord.to_string(),
        },
        Some(p) => Action::Focus {
            pane: p.to_string(),
            direction: direction.to_string(),
        },
        None => Action::FocusCurrent {
            direction: direction.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_to_chord_maps_all_four() {
        assert_eq!(direction_to_chord("left"), Some("ctrl+h"));
        assert_eq!(direction_to_chord("down"), Some("ctrl+j"));
        assert_eq!(direction_to_chord("up"), Some("ctrl+k"));
        assert_eq!(direction_to_chord("right"), Some("ctrl+l"));
    }

    #[test]
    fn direction_to_chord_rejects_unknown() {
        assert_eq!(direction_to_chord("sideways"), None);
        assert_eq!(direction_to_chord(""), None);
    }

    #[test]
    fn decide_sendkeys_when_nvim() {
        assert_eq!(
            decide(Some("wW:p8"), true, "left", "ctrl+h"),
            Action::SendKeys {
                pane: "wW:p8".into(),
                chord: "ctrl+h".into()
            }
        );
    }

    #[test]
    fn decide_focus_when_not_nvim() {
        assert_eq!(
            decide(Some("wW:p8"), false, "left", "ctrl+h"),
            Action::Focus {
                pane: "wW:p8".into(),
                direction: "left".into()
            }
        );
    }

    #[test]
    fn decide_focus_current_when_no_pane() {
        assert_eq!(
            decide(None, false, "left", "ctrl+h"),
            Action::FocusCurrent {
                direction: "left".into()
            }
        );
    }
}
