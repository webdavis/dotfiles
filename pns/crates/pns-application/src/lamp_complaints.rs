use crate::{LampComplaint, LampComplaints};

/// Say a complaint ONCE, and say it again only when it changes.
///
/// THE MARKER IS A PARAMETER because two paths say things at different rates
/// about different sets: the tick folds every refusal of a pass into one line,
/// and the event path says only what it read off the ad-hoc quiet file. Sharing
/// one memory would have each of them forgetting the other's line and repeating
/// it, which is the chatter this whole mechanism exists to stop.
pub fn report_lamp_complaints(
    memory: &impl LampComplaints,
    kind: LampComplaint,
    complaints: &[String],
    mut report: impl FnMut(&str),
) {
    let remembered = memory.remembered(kind);
    match pns_domain::lights::phase::say(complaints, remembered.trim_end_matches('\n')) {
        pns_domain::lights::phase::Say::Nothing => {}
        pns_domain::lights::phase::Say::Aloud(said) => {
            for complaint in complaints {
                report(complaint);
            }
            memory.remember(kind, Some(&said));
        }
        pns_domain::lights::phase::Say::Forget => {
            memory.remember(kind, None);
        }
    }
}

#[cfg(test)]
mod tests;
