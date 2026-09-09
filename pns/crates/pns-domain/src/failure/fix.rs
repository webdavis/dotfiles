//! What to do next, named for where the reader is standing.
//!
//! `fix` names something the reader can ACT ON from where they are. At the
//! machine that is a command; on a phone it is not, because there is no
//! terminal in reach and no supported way to open one from a card. A line that
//! tells a phone reader to run something is worse than saying nothing: it reads
//! as an instruction they cannot follow.

use super::Failure;
use super::meaning::{DESTINATION_HERMES, HERMES_KEY, MOBILE_TOKEN};
use crate::retry::DeliveryOutcome;

/// Where the reader is standing when they read this.
///
/// The phone variant carries what it needs to choose a pointer, because on a
/// phone the fix line is a pointer rather than a repair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// The terminal, `pns doctor`, `pns failures`, the log: the reader is
    /// already where the repair happens.
    Terminal,
    /// A surface under the notification budget.
    Notification(NotificationSurface),
}

/// The surfaces the notification form is written for, and ONLY those.
///
/// Kept apart from [`Surface::Terminal`] because the terminal repair names the
/// route inside the fix line, so it can exceed the whole notification budget by
/// itself, and the fix line is the one thing the notification form will not cut.
/// A caller cannot hand the notification form a terminal surface, rather than
/// being told in a comment not to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationSurface {
    /// The desktop banner. A terminal is one keystroke away, so the line points
    /// at the command rather than carrying the whole repair.
    Banner,
    /// The phone card.
    Phone {
        /// `[failures] serve`. pns cannot detect a moshi Pro subscription and
        /// does not need to: an operator who cannot use browser preview sets
        /// this false, and that is the same switch that decides which pointer
        /// the phone gets.
        serve: bool,
        /// Whether the hermes leg is the one that failed. When it is, the full
        /// form is not in Discord either, so there is nothing to point at.
        hermes_failed: bool,
    },
}

impl From<NotificationSurface> for Surface {
    fn from(surface: NotificationSurface) -> Self {
        Surface::Notification(surface)
    }
}

/// The `fix` line.
pub(super) fn line(failure: &Failure, surface: Surface) -> String {
    // A temporary failure has nothing to repair, so it does not borrow the
    // permanent form's line. Standing down is the same instruction on every
    // surface, which is what makes the two classes tell apart at a glance:
    // permanent says go and do something, temporary says pns has it.
    if !failure.class().is_permanent() {
        return format!(
            "nothing to do, pns will retry ({} of {} attempts used)",
            failure.retries, failure.max_attempts
        );
    }
    let surface = match surface {
        Surface::Terminal => return repair(failure),
        Surface::Notification(surface) => surface,
    };
    match surface {
        NotificationSurface::Banner => {
            "run `pns failures` for the full error and how to fix it".to_string()
        }
        // NAME THE STEPS, not just the destination. moshi supports no deep
        // link, so a bare mention of a page they cannot open is worse than
        // saying nothing; this is something someone can follow with two taps
        // while holding the phone.
        NotificationSurface::Phone { serve: true, .. } => {
            "open moshi's servers list, pick pns :8646".to_string()
        }
        NotificationSurface::Phone {
            serve: false,
            hermes_failed: false,
        } => "full error in Discord, #priority".to_string(),
        // THE HONEST FLOOR. The gateway leg is what broke and there is no page,
        // so a phone reader cannot be handed the record by any route. The line
        // says where it will be rather than pretending otherwise, which is the
        // rule about never reporting a failure through the destination that
        // failed, applied to the pointer.
        NotificationSurface::Phone {
            serve: false,
            hermes_failed: true,
        } => "run `pns failures` on dresden".to_string(),
    }
}

/// The repair itself, for a reader who is already at the machine. It names the
/// FILE to edit, because the next thing that reader does is open one.
fn repair(failure: &Failure) -> String {
    let route = &failure.route;
    if failure.destination != DESTINATION_HERMES {
        return match failure.outcome {
            DeliveryOutcome::Status(401) => {
                format!("put a current moshi token in {MOBILE_TOKEN}")
            }
            DeliveryOutcome::Status(404) => {
                "check the moshi URL, then restart the moshi daemon".to_string()
            }
            _ => "run `pns doctor` for the full mobile configuration".to_string(),
        };
    }
    match failure.outcome {
        DeliveryOutcome::Status(400 | 422) => {
            "report this: pns built a body hermes will not take, which is a pns bug".to_string()
        }
        DeliveryOutcome::Status(401) => {
            format!("put the gateway's current key in {HERMES_KEY}")
        }
        DeliveryOutcome::Status(403) => {
            format!("grant the {HERMES_KEY} access to {route} in ~/.hermes/config.yaml")
        }
        DeliveryOutcome::Status(404 | 410) => format!(
            "run `pns doctor` to see which routes the gateway accepts, \
             then add \"{route}\" to ~/.hermes/config.yaml"
        ),
        DeliveryOutcome::Status(405) => {
            format!("make \"{route}\" a POST route in ~/.hermes/config.yaml")
        }
        DeliveryOutcome::Status(413) => {
            "raise the gateway's body limit in ~/.hermes/config.yaml".to_string()
        }
        DeliveryOutcome::NoStatus => {
            format!("check the route name \"{route}\" and the URL in ~/.config/pns/config.toml")
        }
        _ => "run `pns doctor` for the full gateway configuration".to_string(),
    }
}
