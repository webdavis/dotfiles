use crate::LampMutes;
use pns_domain::lights::mute::QuietCommand;

/// The lamps' own mute: one place, quiet for a bounded while, by hand.
///
/// LIGHTS ONLY, and that is the operator's own scope: cards, banners, the
/// durable log and `pns quiet` are untouched, so an agent that needs an answer
/// still reaches the phone while the bedroom lamp stays out of it. The two
/// mutes share a duration parser and nothing else, and neither reads the
/// other's file.
///
/// FAIL OPEN AT EVERY TURN, which is `quiet.rs`'s direction rather than the
/// window's: a state file nobody can parse mutes NOTHING and says so, because a
/// lights mute the operator cannot see is worse than a lamp that flashed.
///
/// THE READ-MODIFY-WRITE RACE IS REAL AND ACCEPTED. This is hand-typed, so two
/// runs racing means an operator typing two commands in the same second, and
/// the loser is one mute they can see is missing and retype. A lock between two
/// interactive commands would be a mechanism with no reader.
pub struct SetLightsQuiet<'a, M> {
    pub mutes: &'a M,
}
impl<M: LampMutes> SetLightsQuiet<'_, M> {
    pub fn run(
        &self,
        command: &QuietCommand,
        now: Option<u64>,
        mut warn: impl FnMut(&str),
    ) -> Result<Vec<String>, String> {
        let (entries, complaints) = self.mutes.read();
        // SAID BEFORE ANYTHING IS WRITTEN, because the write below republishes the
        // whole file: an operator whose file was unreadable is losing whatever it
        // held, and that is a line they get to see rather than a silent repair.
        for complaint in &complaints {
            warn(complaint);
        }
        let rebuilt = match &command {
            pns_domain::lights::mute::QuietCommand::Report => Ok(entries.clone()),
            pns_domain::lights::mute::QuietCommand::Unmute { place } => {
                pns_domain::lights::mute::muted_after(&entries, place, None, now)
            }
            pns_domain::lights::mute::QuietCommand::Mute { place, seconds } => {
                match now.map(|now| now.saturating_add(*seconds)) {
                    Some(expiry) => {
                        pns_domain::lights::mute::muted_after(&entries, place, Some(expiry), now)
                    }
                    // THE CLOCK IS WHAT A MUTE IS MADE OF, so a run that cannot
                    // read one says the mute was not set rather than writing an
                    // expiry it guessed. `pns quiet`'s own wording, one file over.
                    None => Err(
                        "pns: state error (the clock cannot be read); the mute was not set"
                            .to_string(),
                    ),
                }
            }
        };
        // A REFUSED REBUILD IS A MUTE THAT WAS NOT SET, and nothing is written or
        // reported after one: the file on disk is exactly what it was, and a report
        // built from a list this run refused to publish would describe a house that
        // does not exist.
        let kept = rebuilt?;
        if !matches!(command, pns_domain::lights::mute::QuietCommand::Report)
            && let Err(error) = self.mutes.write(&kept)
        {
            // LOUD, because a human is waiting on the answer: reporting a mute that
            // is not in effect is the worst outcome available.
            let refusal = format!(
                "pns: state error (lights-quiet could not be written: {error}); \
             the mute was not set"
            );
            // AND NO REPORT AFTER IT. `kept` is what the file WOULD have held: for
            // a failed mute it would say the place is quiet when it is not, and for
            // a failed `off` it would say nothing is quiet while the old mute is
            // still on disk and still taking the lamp. The disk is the answer and
            // this run did not change it.
            return Err(refusal);
        }
        Ok(pns_domain::lights::mute::muted_report(&kept, now))
    }
}

mod names;
pub use names::quiet_names;

mod reading;
pub use reading::ad_hoc_quiet;

#[cfg(test)]
mod tests;
