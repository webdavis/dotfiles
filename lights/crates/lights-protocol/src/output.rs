#[derive(Debug, PartialEq)]
pub enum Output<'a> {
    Power {
        room: &'a str,
        on: bool,
    },
    Absolute {
        room: &'a str,
        percent: u8,
    },
    Step {
        room: &'a str,
        up: bool,
    },
    Scene {
        room: &'a str,
        scene: &'a str,
    },
    Status {
        room: &'a str,
        on: bool,
        brightness: Option<f64>,
        scene: Option<&'a str>,
    },
}
impl Output<'_> {
    pub fn render(&self) -> String {
        let line = match self {
            Self::Power { room, on } => format!("{room}: {}", if *on { "on" } else { "off" }),
            Self::Absolute { room, percent } => format!("{room}: brightness requested {percent}%"),
            Self::Step { room, up } => {
                format!("{room}: brightness {}", if *up { "up" } else { "down" })
            }
            Self::Scene { room, scene } => format!("Room: {room} | Scene: {scene}"),
            Self::Status {
                room,
                on,
                brightness,
                scene,
            } => format!(
                "{room}: {} | brightness: {} | scene: {}",
                if *on { "ON" } else { "OFF" },
                brightness
                    .map(|n| format!("{n}%"))
                    .unwrap_or_else(|| "unknown".into()),
                scene.unwrap_or("unknown")
            ),
        };
        format!("{line}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absolute_output_labels_requested_percentage() {
        assert_eq!(
            Output::Absolute {
                room: "Studio",
                percent: 1
            }
            .render(),
            "Studio: brightness requested 1%\n"
        );
    }
    #[test]
    fn relative_output_has_no_result_percentage() {
        for (up, expected) in [(true, "up"), (false, "down")] {
            assert_eq!(
                Output::Step { room: "Studio", up }.render(),
                format!("Studio: brightness {expected}\n")
            );
        }
    }
    #[test]
    fn status_output_preserves_reported_fractional_brightness() {
        assert_eq!(
            Output::Status {
                room: "Studio",
                on: true,
                brightness: Some(42.75),
                scene: Some("Read")
            }
            .render(),
            "Studio: ON | brightness: 42.75% | scene: Read\n"
        );
    }
    #[test]
    fn status_without_static_scene_prints_unknown() {
        assert_eq!(
            Output::Status {
                room: "Studio",
                on: false,
                brightness: None,
                scene: None
            }
            .render(),
            "Studio: OFF | brightness: unknown | scene: unknown\n"
        );
    }
}
