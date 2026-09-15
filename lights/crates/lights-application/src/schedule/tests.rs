use super::*;
use lights_domain::PresetWindow;

struct Stopped(Option<MinuteOfDay>);
impl Clock for Stopped {
    fn local_minute(&self) -> Option<MinuteOfDay> {
        self.0
    }
}

fn windows() -> PresetWindows {
    PresetWindows::new(vec![
        PresetWindow::new(6 * 60, 12 * 60, "energize".into()).unwrap(),
        PresetWindow::new(22 * 60, 6 * 60, "nightlight".into()).unwrap(),
    ])
}

#[test]
fn a_minute_inside_a_window_resolves_that_windows_preset() {
    assert_eq!(
        ChoosePreset::run(&Stopped(Some(7 * 60)), &windows()),
        Ok("energize")
    );
    assert_eq!(
        ChoosePreset::run(&Stopped(Some(23 * 60)), &windows()),
        Ok("nightlight")
    );
}

#[test]
fn a_minute_no_window_covers_is_refused_with_the_minute() {
    assert_eq!(
        ChoosePreset::run(&Stopped(Some(15 * 60)), &windows()),
        Err(NoPresetNow::Uncovered(15 * 60))
    );
}

#[test]
fn no_configured_windows_is_refused_before_the_clock_is_read() {
    assert_eq!(
        ChoosePreset::run(&Stopped(None), &PresetWindows::default()),
        Err(NoPresetNow::NoWindows)
    );
}

#[test]
fn a_clock_that_cannot_answer_is_refused_rather_than_guessed() {
    assert_eq!(
        ChoosePreset::run(&Stopped(None), &windows()),
        Err(NoPresetNow::ClockUnavailable)
    );
}
