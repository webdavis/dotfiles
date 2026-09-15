use crate::ValueError;

/// A minute of the local day, 0 through 1439. The clock is read once, at the
/// edge, and every rule below is a function of this number: no zone, no date
/// and no calendar arithmetic reaches the domain.
pub type MinuteOfDay = u16;

pub const MINUTES_PER_DAY: MinuteOfDay = 24 * 60;

/// One stretch of the day and the preset it asks for. The start is inclusive
/// and the end exclusive, so adjacent windows written `06:00-12:00` and
/// `12:00-17:00` neither overlap nor leave a minute uncovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresetWindow {
    start: MinuteOfDay,
    end: MinuteOfDay,
    preset: String,
}

impl PresetWindow {
    pub fn new(start: MinuteOfDay, end: MinuteOfDay, preset: String) -> Result<Self, ValueError> {
        if start >= MINUTES_PER_DAY || end >= MINUTES_PER_DAY {
            return Err(ValueError("window time must be within one day"));
        }
        // A window that starts where it ends covers either nothing or
        // everything, and which one was meant is unknowable, so it is refused
        // rather than guessed.
        if start == end {
            return Err(ValueError("window start and end must differ"));
        }
        if preset.trim().is_empty() {
            return Err(ValueError("window must name a preset"));
        }
        Ok(Self { start, end, preset })
    }
    pub fn preset(&self) -> &str {
        &self.preset
    }
    /// An end BEFORE the start is the wrap past midnight, which is the shape
    /// every night window has, so it is read as two stretches rather than
    /// refused: from the start to midnight, and from midnight to the end.
    fn covers(&self, minute: MinuteOfDay) -> bool {
        if self.start < self.end {
            (self.start..self.end).contains(&minute)
        } else {
            minute >= self.start || minute < self.end
        }
    }
}

/// The windows in the order the operator wrote them. ORDER IS THE TIE-BREAK:
/// the first window covering the minute wins, so an overlap is resolved by
/// moving a line rather than by a precedence rule nobody can see.
#[derive(Debug, Default)]
pub struct PresetWindows(Vec<PresetWindow>);

impl PresetWindows {
    pub fn new(windows: Vec<PresetWindow>) -> Self {
        Self(windows)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn preset_at(&self, minute: MinuteOfDay) -> Option<&str> {
        self.0
            .iter()
            .find(|window| window.covers(minute))
            .map(PresetWindow::preset)
    }
    pub fn presets(&self) -> impl Iterator<Item = &str> {
        self.0.iter().map(PresetWindow::preset)
    }
}

#[cfg(test)]
mod tests;
