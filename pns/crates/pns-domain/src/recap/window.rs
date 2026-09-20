//! The windows a recap is taken over: the day's four periods, `today`, `week`,
//! and one step back from any of them.
//!
//! PURE CALENDAR ARITHMETIC, and that is what makes it a domain module. Which
//! epoch second a local moment falls on is a system fact (a zone database, a
//! `TZ` variable, two transitions a year), so this answers in LOCAL CIVIL
//! MOMENTS and the composition root resolves each one through `local_epoch`.
//! Stepping a day back by subtracting 86,400 seconds would be an hour wrong
//! twice a year; stepping the DATE back and resolving the new date is right on
//! both sides of a transition.

/// A moment on the operator's own calendar, before any zone is applied to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalCivilTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl LocalCivilTime {
    /// The same calendar day at a minute of it, seconds zeroed. A window bound
    /// is stated to the minute, which is the width `[recap]`'s own `HH:MM`
    /// values carry.
    fn at(self, minutes: u32) -> Self {
        LocalCivilTime {
            hour: minutes / 60,
            minute: minutes % 60,
            second: 0,
            ..self
        }
    }
    /// The same time of day `days` later, or earlier for a negative count.
    fn plus_days(self, days: i64) -> Self {
        let (year, month, day) = civil_from_days(days_from_civil(self) + days);
        LocalCivilTime {
            year,
            month,
            day,
            ..self
        }
    }
    /// Minutes since this day's local midnight.
    fn minutes(self) -> u32 {
        self.hour * 60 + self.minute
    }
}

/// One of the day's periods, as minutes since local midnight. `start > end`
/// is a period that crosses midnight, which `overnight` does at its default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Period {
    pub start: u32,
    pub end: u32,
}

impl Period {
    /// Whether this period runs past midnight into the next day.
    fn crosses_midnight(self) -> bool {
        self.start > self.end
    }
}

/// The four periods, which must tile the day between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Periods {
    pub overnight: Period,
    pub morning: Period,
    pub afternoon: Period,
    pub evening: Period,
}

impl Default for Periods {
    fn default() -> Self {
        Periods {
            overnight: Period {
                start: 22 * 60,
                end: 6 * 60,
            },
            morning: Period {
                start: 6 * 60,
                end: 12 * 60,
            },
            afternoon: Period {
                start: 12 * 60,
                end: 17 * 60,
            },
            evening: Period {
                start: 17 * 60,
                end: 22 * 60,
            },
        }
    }
}

impl Periods {
    /// The four, in the order a fault names them and a scan walks them.
    fn each(&self) -> [(Window, Period); 4] {
        [
            (Window::Overnight, self.overnight),
            (Window::Morning, self.morning),
            (Window::Afternoon, self.afternoon),
            (Window::Evening, self.evening),
        ]
    }
    fn of(&self, window: Window) -> Option<Period> {
        self.each()
            .into_iter()
            .find_map(|(named, period)| (named == window).then_some(period))
    }
}

/// Which day a week starts on, for `week` and `last-week`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WeekStart {
    #[default]
    Monday,
    Sunday,
}

impl WeekStart {
    /// The word config spells it with, which is also what a refusal lists.
    pub fn as_str(self) -> &'static str {
        match self {
            WeekStart::Monday => "monday",
            WeekStart::Sunday => "sunday",
        }
    }
    pub fn parse(word: &str) -> Option<Self> {
        [WeekStart::Monday, WeekStart::Sunday]
            .into_iter()
            .find(|start| start.as_str() == word)
    }
    /// How many days back this week started, given a weekday where 0 is
    /// Sunday, which is the numbering `tm_wday` uses.
    fn days_back(self, weekday: u32) -> i64 {
        match self {
            WeekStart::Sunday => i64::from(weekday),
            WeekStart::Monday => i64::from((weekday + 6) % 7),
        }
    }
}

/// A window a recap may be taken over. `yesterday` and `last-week` are not
/// here: they are `today` and `week` one step back, which is what the design
/// says they are, so the same arithmetic answers all four.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Window {
    Overnight,
    Morning,
    Afternoon,
    Evening,
    Today,
    Week,
}

impl Window {
    /// The word this window is typed and reported as.
    pub fn as_str(self) -> &'static str {
        match self {
            Window::Overnight => "overnight",
            Window::Morning => "morning",
            Window::Afternoon => "afternoon",
            Window::Evening => "evening",
            Window::Today => "today",
            Window::Week => "week",
        }
    }
}

/// The window one word names, and whether the word itself means one step back.
///
/// `yesterday` AND `last-week` ARE SHORTCUTS rather than windows of their own,
/// exactly as the design states, so `pns recap yesterday --previous` is the day
/// before yesterday and needs no rule of its own to be.
pub fn parse_window(word: &str) -> Option<(Window, bool)> {
    Some(match word {
        "overnight" => (Window::Overnight, false),
        "morning" => (Window::Morning, false),
        "afternoon" => (Window::Afternoon, false),
        "evening" => (Window::Evening, false),
        "today" => (Window::Today, false),
        "week" => (Window::Week, false),
        "yesterday" => (Window::Today, true),
        "last-week" => (Window::Week, true),
        _ => return None,
    })
}

/// Every window word, for the sentence a refusal prints.
pub const WINDOW_WORDS: &[&str] = &[
    "overnight",
    "morning",
    "afternoon",
    "evening",
    "today",
    "yesterday",
    "week",
    "last-week",
];

/// The gap or the overlap the four periods leave, named, or None when they
/// tile the day.
///
/// BOTH WINDOWS ARE NAMED, which is the whole value of the check: "overnight
/// ends at 06:00 and morning starts at 06:30" is a sentence the operator can
/// act on, where "the recap windows do not tile the day" sends them looking
/// through four pairs for the one that moved.
pub fn tiling_fault(periods: &Periods) -> Option<String> {
    let mut ordered = periods.each();
    ordered.sort_by_key(|(_, period)| period.start);
    for index in 0..ordered.len() {
        let (name, period) = ordered[index];
        let (next_name, next) = ordered[(index + 1) % ordered.len()];
        if period.end != next.start {
            return Some(format!(
                "`recap` window `{}` ends at {} and `{}` starts at {}, \
                 so the four windows leave the day uncovered",
                name.as_str(),
                written(period.end),
                next_name.as_str(),
                written(next.start),
            ));
        }
    }
    None
}

/// One minute of the day back in the `HH:MM` the config states it in.
fn written(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// The period whose end passed most recently, which is what a bare
/// `pns recap` is about.
///
/// THE DISTANCE IS MEASURED AROUND THE CLOCK, so a moment before every end of
/// the day still answers: at 03:00 no period has ended yet today and the one
/// that ended most recently is yesterday evening's, which `(minutes - end) mod
/// 1440` says without a case of its own.
pub fn most_recently_ended(periods: &Periods, minutes: u32) -> Window {
    periods
        .each()
        .into_iter()
        .min_by_key(|(_, period)| (minutes + 1440 - period.end) % 1440)
        .map(|(name, _)| name)
        .unwrap_or(Window::Overnight)
}

/// The bounds of one window instance, as local civil moments the caller
/// resolves through the zone.
///
/// A NAMED PERIOD MEANS ITS MOST RECENT INSTANCE, in progress or complete,
/// which is the design's own rule: `morning` at 10:00 is this morning so far
/// and at 15:00 it is this morning complete. `previous` steps back exactly one
/// instance, which is one day for a period and for `today`, and seven for
/// `week`.
///
/// `today` AND `week` END AT NOW rather than at midnight, because both are
/// open windows; one step back closes them, since yesterday and last week are
/// over.
pub fn resolve(
    window: Window,
    previous: bool,
    periods: &Periods,
    week_start: WeekStart,
    now: LocalCivilTime,
    weekday: u32,
) -> (LocalCivilTime, LocalCivilTime) {
    let step = i64::from(previous);
    match window {
        Window::Today => {
            let midnight = now.at(0).plus_days(-step);
            let until = match previous {
                true => midnight.plus_days(1),
                false => now,
            };
            (midnight, until)
        }
        Window::Week => {
            let start = now
                .at(0)
                .plus_days(-week_start.days_back(weekday) - 7 * step);
            let until = match previous {
                true => start.plus_days(7),
                false => now,
            };
            (start, until)
        }
        named => {
            let period = periods.of(named).unwrap_or(Period { start: 0, end: 0 });
            period_instance(period, now, step)
        }
    }
}

/// One instance of a period, `step` days back from the most recent one.
///
/// THE DAY IS CHOSEN OFF THE START, not off the end: the instance in progress
/// or most recently complete is the one whose start has already passed. A
/// period crossing midnight is the same rule with its end on the next day.
fn period_instance(
    period: Period,
    now: LocalCivilTime,
    step: i64,
) -> (LocalCivilTime, LocalCivilTime) {
    let started_today = now.minutes() >= period.start;
    let day = now.plus_days(if started_today { -step } else { -step - 1 });
    let since = day.at(period.start);
    let until = day
        .plus_days(i64::from(period.crosses_midnight()))
        .at(period.end);
    (since, until)
}

/// Days since 1970-01-01 for a civil date, Howard Hinnant's `days_from_civil`.
/// PROLEPTIC GREGORIAN, which is what every calendar this runs against uses.
fn days_from_civil(date: LocalCivilTime) -> i64 {
    let year = i64::from(date.year) - i64::from(date.month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let month = i64::from(date.month);
    let day_of_year =
        (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + i64::from(date.day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// The civil date a day count names, `days_from_civil` run backwards.
fn civil_from_days(days: i64) -> (u32, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    (
        u32::try_from(year + i64::from(month <= 2)).unwrap_or_default(),
        u32::try_from(month).unwrap_or_default(),
        u32::try_from(day).unwrap_or_default(),
    )
}

#[cfg(test)]
#[path = "window/tests.rs"]
mod tests;
