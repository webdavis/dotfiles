use crate::{Clock, HeldLamps, LampComplaint, LampComplaints, LampMutes};
use pns_domain::lamps::{QuietWindow, Reading, config::Lights, quiet_now};

pub fn signal_after_delivery(
    clock: &impl Clock,
    minutes: impl FnOnce(u64) -> Option<u16>,
    lights: Option<&Lights>,
    window: impl FnOnce() -> Result<Option<QuietWindow>, String>,
    rooms: impl FnOnce(),
    mapped: impl FnOnce(&Lights, Option<u64>, Option<u16>),
    mut report: impl FnMut(&str),
) {
    // FRESH, not the run's start: the legs above dial the network under their
    // own deadlines, so a run can cross into a dim window between starting and
    // reaching the moment a lamp would actually light, and the older reading
    // would flash it just inside quiet hours. The clock is injected so the gate can be observed after delivery advances it.
    let now = clock.now_secs();
    let minutes_now = now.and_then(minutes);
    if let Some(lights) = lights {
        mapped(lights, now, minutes_now);
    } else {
        // TODAY'S PATH, UNCHANGED, and it is the compatibility claim of this
        // whole change: one house window for the whole pulse, one write per room
        // in `[plugins.hue] rooms`, and one refusal that costs the pulse when
        // nobody can read the window. A machine that never wrote a `[lights]`
        // table reaches nothing new.
        match window() {
            Ok(window) => {
                if !quiet_now(window.as_ref(), minutes_now) {
                    rooms();
                }
            }
            // FAIL CLOSED, the direction the pulse takes on every unreadable
            // reading: a window nobody can parse is an operator who asked for
            // quiet hours and cannot be told which ones, so the room stays
            // dark and the refusal says why.
            Err(refusal) => report(&refusal),
        }
    }
}

pub fn signal_mapped<R: LampMutes + HeldLamps + LampComplaints>(
    records: &R,
    now: Option<u64>,
    minutes_now: Option<u16>,
    write: impl FnOnce(&Reading<'_>, Option<&[String]>) -> Vec<String>,
    report: impl FnMut(&str),
) {
    // THE OPERATOR'S OWN AD-HOC QUIET, read here rather than inside the walk
    // for the reason every reading on this path is: the modules take no files
    // and no clock, and the composition root decides where a complaint goes.
    // A machine that has never typed the command reads no file and pays one
    // failed open.
    let (muted, mut complaints) = crate::ad_hoc_quiet(records, now);
    let held = HeldLamps::read(records).map(|entries| {
        entries
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>()
    });
    complaints.extend(write(
        &Reading {
            minutes_now,
            muted: &muted,
        },
        held.as_deref(),
    ));
    // SAY-ONCE, NOT ONCE PER EVENT. A state file something else corrupted stays
    // corrupt until a human fixes it, and this path fires many times a session,
    // so a bare print here is one stderr line per hook invocation forever.
    //
    // AND IT CARRIES THE RESOLUTION'S OWN FINDINGS TOO, which used to be
    // discarded here. A machine whose map routes only `done` and `failed` holds
    // no state, so its tick never resolves anything and never complains: a
    // mistyped lamp name on such a config was dark forever with the whole
    // system silent about it, and this is the path that meets it.
    crate::report_lamp_complaints(records, LampComplaint::Quiet, &complaints, report);
}

#[cfg(test)]
mod tests;
