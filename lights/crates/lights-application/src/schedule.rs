use lights_domain::{MinuteOfDay, PresetWindows};

/// The wall clock, as the minute of the LOCAL day. A minute is all any rule
/// here needs, and asking for only that keeps the zone database, the date and
/// the two transitions a year on the far side of the boundary.
pub trait Clock {
    /// `None` when the system cannot say which minute it is, which is a
    /// refusal rather than a reason to pick a preset at random.
    fn local_minute(&self) -> Option<MinuteOfDay>;
}

/// Why the clock named no preset. Each arm is a different thing for the
/// operator to do, which is why they are not one message.
#[derive(Debug, PartialEq, Eq)]
pub enum NoPresetNow {
    NoWindows,
    ClockUnavailable,
    Uncovered(MinuteOfDay),
}

/// Picks the preset the configured windows give the current minute.
pub struct ChoosePreset;
impl ChoosePreset {
    pub fn run<'a>(clock: &dyn Clock, windows: &'a PresetWindows) -> Result<&'a str, NoPresetNow> {
        if windows.is_empty() {
            return Err(NoPresetNow::NoWindows);
        }
        let minute = clock.local_minute().ok_or(NoPresetNow::ClockUnavailable)?;
        windows
            .preset_at(minute)
            .ok_or(NoPresetNow::Uncovered(minute))
    }
}

#[cfg(test)]
mod tests;
