//! The two forms. Both carry the same fields in the same order.

use super::fix::{self, NotificationSurface, Surface};
use super::meaning;
use super::{Failure, NOTIFICATION_MAX_CHARS};

/// The fields, in order, and their labels in the full form. One list, so the
/// two forms cannot drift into a different order.
const LABELS: [&str; 5] = [
    "status",
    "meaning",
    "webhook route",
    "failed command",
    "fix",
];

/// The label column of the full form, wide enough for the longest label plus
/// its colon and one space.
const LABEL_WIDTH: usize = 16;

/// The indent a wrapped `fix` line hangs under, so it lines up with the text
/// above it rather than with the labels.
const HANGING_INDENT: usize = LABEL_WIDTH + 2;

/// The full form: for surfaces with no length limit and a monospace face.
///
/// This is the only form that carries `sent by`. The notification form drops it
/// because `failed command` already contains `--agent <name>`, and a form under
/// a character budget cannot afford to say anything twice.
pub fn full(failure: &Failure) -> String {
    let fix = fix::line(failure, Surface::Terminal);
    let mut out = String::from("pns: delivery failed\n");
    for (label, value) in LABELS.iter().zip([
        meaning::status(failure.outcome),
        meaning::meaning(failure),
        failure.route.clone(),
        failure.command.clone(),
        fix,
    ]) {
        // `sent by` sits between the route and the command, where a reader
        // looking for who sent this finds it before the how.
        if *label == "failed command" {
            out.push_str(&row("sent by", &failure.agent));
        }
        out.push_str(&row(label, &value));
    }
    out
}

fn row(label: &str, value: &str) -> String {
    let head = format!("{label}:");
    let mut lines = value.split('\n');
    let first = lines.next().unwrap_or_default();
    let mut out = format!("  {head:<LABEL_WIDTH$}{first}\n");
    for line in lines {
        out.push_str(&format!("{:HANGING_INDENT$}{line}\n", ""));
    }
    out
}

/// The notification form: for the desktop banner and the phone card.
///
/// Two differences from the full form, both forced by the budget. There is no
/// column padding, because banners and cards render proportionally and padding
/// produces ragged text rather than alignment. And `sent by` is dropped, for
/// the reason above.
///
/// The gateway is named once, inside `meaning`, which is why `webhook route`
/// carries the bare name.
///
/// THE BUDGET IS HELD BY THIS FUNCTION, not by whoever calls it. A route name
/// or a producer's command is as long as the producer made it, and a body that
/// goes over is cut by the surface at whatever character it reaches, which
/// takes the `fix` line off the end: the one line the reader is meant to act
/// on. So the room is spent in order of what the reader can least afford to
/// lose, and the fields that give way say they were cut.
pub fn notification(failure: &Failure, surface: NotificationSurface) -> String {
    // `fix` is not in this array, and that is the point: it is the line the
    // reader acts on, and it is pns's own prose with a bounded length, so it
    // never gives way.
    let mut lines = [
        format!("status: {}", meaning::status(failure.outcome)),
        format!("meaning: {}", meaning::meaning(failure)),
        format!("webhook route: {}", failure.route),
        format!("failed command: {}", failure.command),
    ];
    let fix = format!("fix: {}", fix::line(failure, surface.into()));

    // The order the reader can least afford to lose, read backwards: `meaning`
    // is pns's prose and a reader can lose its tail and still act; `command`
    // and `route` are producer text of unbounded length, and either can fill
    // the whole budget on its own; `status` is a handful of characters and only
    // gives way when nothing else is left. Whatever gives way SAYS it was cut,
    // because `clipped` marks it.
    for index in [MEANING, COMMAND, ROUTE, STATUS] {
        let spent = NEWLINES
            + fix.chars().count()
            + lines
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, line)| line.chars().count())
                .sum::<usize>();
        lines[index] =
            crate::render::clipped(&lines[index], NOTIFICATION_MAX_CHARS.saturating_sub(spent));
    }

    let [status, meaning, route, command] = lines;
    [status, meaning, route, command, fix].join("\n")
}

const STATUS: usize = 0;
const MEANING: usize = 1;
const ROUTE: usize = 2;
const COMMAND: usize = 3;

/// The four newlines joining five lines, which count against the budget.
const NEWLINES: usize = 4;
