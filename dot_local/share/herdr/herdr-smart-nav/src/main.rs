use std::env;
use std::process::Command;

fn direction_to_chord(direction: &str) -> Option<&'static str> {
    match direction {
        "left" => Some("ctrl+h"),
        "down" => Some("ctrl+j"),
        "up" => Some("ctrl+k"),
        "right" => Some("ctrl+l"),
        _ => None,
    }
}

fn is_nvim_foreground(process_info_json: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(process_info_json)
        .ok()
        .and_then(|v| {
            v["result"]["process_info"]["foreground_processes"]
                .as_array()
                .map(|procs| procs.iter().any(|p| p["name"].as_str() == Some("nvim")))
        })
        .unwrap_or(false)
}

#[derive(Debug, PartialEq)]
enum Action {
    SendKeys { pane: String, chord: String },
    Focus { pane: String, direction: String },
    FocusCurrent { direction: String },
}

fn decide(pane: Option<&str>, is_nvim: bool, direction: &str, chord: &str) -> Action {
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

// ---- impure boundary (not unit-tested; mirrors last-workspace's Command use) ----

fn herdr_bin() -> String {
    env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

fn herdr_capture(args: &[&str]) -> String {
    Command::new(herdr_bin())
        .args(args)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

fn herdr_run(args: &[&str]) {
    let _ = Command::new(herdr_bin()).args(args).status();
}

fn execute(action: &Action) {
    match action {
        Action::SendKeys { pane, chord } => herdr_run(&["pane", "send-keys", pane, chord]),
        Action::Focus { pane, direction } => {
            herdr_run(&["pane", "focus", "--direction", direction, "--pane", pane])
        }
        Action::FocusCurrent { direction } => {
            herdr_run(&["pane", "focus", "--direction", direction, "--current"])
        }
    }
}

fn main() {
    let direction = env::args().nth(1).unwrap_or_default();
    let Some(chord) = direction_to_chord(&direction) else {
        eprintln!("herdr-smart-nav: usage: herdr-smart-nav left|down|up|right");
        std::process::exit(2);
    };
    let pane = env::var("HERDR_ACTIVE_PANE_ID")
        .ok()
        .filter(|p| !p.is_empty());
    let is_nvim = match pane.as_deref() {
        Some(p) => is_nvim_foreground(&herdr_capture(&["pane", "process-info", "--pane", p])),
        None => false,
    };
    execute(&decide(pane.as_deref(), is_nvim, &direction, chord));
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
    fn is_nvim_foreground_true_when_present() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"},{"name":"nvim"}]}}}"#;
        assert!(is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_when_absent() {
        let j = r#"{"result":{"process_info":{"foreground_processes":[{"name":"bash"}]}}}"#;
        assert!(!is_nvim_foreground(j));
    }

    #[test]
    fn is_nvim_foreground_false_on_garbage() {
        assert!(!is_nvim_foreground("not json"));
        assert!(!is_nvim_foreground("{}"));
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
