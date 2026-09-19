use crate::style::{self, HeaderLine, Paint, Tone};
use pns_protocol::{TapOperation, TapResult};

pub(crate) fn render(result: &TapResult) -> Vec<String> {
    let safe = |text: &str| text.chars().filter(|c| !c.is_control()).collect::<String>();
    if !result.ok && result.operation != TapOperation::Info {
        return vec![format!("pns tap: {}", safe(&result.message))];
    }
    if result.operation == TapOperation::Tap {
        return vec![result.message.clone()];
    }
    let paint = Paint::for_stdout();
    let row = |text: &str| style::row(paint, Tone::Quiet, "·", 2, text);
    let mut lines;
    if let Some(guide) = &result.install {
        lines = style::header(
            paint,
            "pns tap install",
            &[
                HeaderLine {
                    label: "1. This Mac",
                    text: "the authorized_keys line",
                },
                HeaderLine {
                    label: "2. Your phone",
                    text: "the PNS Tap shortcut",
                },
                HeaderLine {
                    label: "3. Trigger methods",
                    text: "Back Tap, Action Button, others",
                },
            ],
        );
        for step in &guide.steps {
            lines.push(String::new());
            lines.push(style::heading(paint, &step.title, &step.blurb));
            lines.extend(step.lines.iter().map(|line| row(line)));
        }
        lines.push(String::new());
        lines.push(style::heading(
            paint,
            "Undo",
            "remove only this integration",
        ));
        lines.extend(guide.undo.iter().map(|line| row(line)));
    } else {
        lines = style::header(
            paint,
            "pns tap info",
            &[HeaderLine {
                label: "About",
                text: "a tap supplies the phone's attention signal",
            }],
        );
        if let Some(marker) = &result.marker {
            lines.push(row(&format!("Marker: {:?}", marker.path)));
            let source = match marker.source.as_str() {
                "config" => "[phone] marker_file",
                _ => "shipped default",
            };
            lines.push(row(&format!("Source: {source}")));
            lines.push(row(&format!("Config file: {:?}", marker.config_file)));
            lines.push(row(&format!(
                "Exists: {}",
                match marker.exists {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unknown",
                }
            )));
            lines.push(row(&match (marker.exists, marker.age_secs) {
                (Some(false), _) => "Last tap: never tapped".into(),
                (_, Some(age)) => format!("Last tap: {age} seconds ago"),
                _ => "Last tap: unknown".into(),
            }));
            lines.push(row(&format!(
                "Fresh: {}",
                match marker.fresh {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "unknown",
                }
            )));
        }
        lines.push(row(&safe(&result.message)));
        lines.push(row("To undo a tap, delete the marker file. Nothing else on this Mac changes, and the surface reads as untapped again."));
        lines.push(String::new());
        lines.push(style::heading(
            paint,
            "Troubleshooting",
            "when a tap does not arrive",
        ));
        lines.push(row("An asleep or unreachable Mac may never receive the tap. Check Remote Login (System Settings, General, Sharing, Remote Login) and connectivity, then retry when the Mac is awake."));
        lines.push(row("A Mac that cannot answer fails the Shortcut's SSH action, so the phone shows that SSH error and never the success notification."));
        lines.push(row("Local commands can also update this marker; verify the phone path by making a tap from the phone."));
    }
    lines.push(String::new());
    lines.push(style::rule(paint));
    lines.push(format!(
        "  {}",
        if result.operation == TapOperation::Install {
            "Check the result with pns tap info, then test from the phone."
        } else {
            "Setup guide: pns tap install"
        }
    ));
    lines
}
