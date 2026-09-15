use super::*;

fn window(start: MinuteOfDay, end: MinuteOfDay, preset: &str) -> PresetWindow {
    PresetWindow::new(start, end, preset.into()).unwrap()
}

#[test]
fn a_window_covers_its_start_and_stops_before_its_end() {
    let windows = PresetWindows::new(vec![window(6 * 60, 12 * 60, "energize")]);
    assert_eq!(windows.preset_at(6 * 60), Some("energize"));
    assert_eq!(windows.preset_at(9 * 60 + 30), Some("energize"));
    assert_eq!(windows.preset_at(12 * 60), None);
    assert_eq!(windows.preset_at(5 * 60 + 59), None);
}

#[test]
fn a_window_ending_before_it_starts_wraps_past_midnight() {
    let windows = PresetWindows::new(vec![window(22 * 60, 6 * 60, "nightlight")]);
    assert_eq!(windows.preset_at(23 * 60), Some("nightlight"));
    assert_eq!(windows.preset_at(0), Some("nightlight"));
    assert_eq!(windows.preset_at(5 * 60 + 59), Some("nightlight"));
    assert_eq!(windows.preset_at(6 * 60), None);
    assert_eq!(windows.preset_at(12 * 60), None);
}

#[test]
fn overlapping_windows_resolve_to_the_first_one_written() {
    let windows = PresetWindows::new(vec![
        window(8 * 60, 10 * 60, "first"),
        window(9 * 60, 11 * 60, "second"),
    ]);
    assert_eq!(windows.preset_at(9 * 60), Some("first"));
    assert_eq!(windows.preset_at(10 * 60), Some("second"));
}

#[test]
fn a_window_outside_the_day_or_naming_nothing_is_refused() {
    for (start, end, preset) in [
        (MINUTES_PER_DAY, 60, "bed"),
        (60, MINUTES_PER_DAY, "bed"),
        (60, 60, "bed"),
        (60, 120, "  "),
    ] {
        assert!(PresetWindow::new(start, end, preset.into()).is_err());
    }
}
