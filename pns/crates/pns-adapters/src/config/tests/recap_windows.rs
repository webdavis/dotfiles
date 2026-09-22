use super::*;
use pns_domain::recap::window::Period;

#[test]
fn the_nightshift_key_moves_the_night_window() {
    let config = parse_config(
        "[recap]\nnightshift = [\"23:00\", \"06:00\"]\nevening = [\"17:00\", \"23:00\"]\n",
    )
    .unwrap();
    assert_eq!(
        config.recap.periods.nightshift,
        Period {
            start: 23 * 60,
            end: 6 * 60
        }
    );
}

#[test]
fn a_nightshift_value_that_is_no_window_is_refused_naming_the_key() {
    assert_eq!(
        refusal("[recap]\nnightshift = [\"22:00\"]\n"),
        "`recap` key `nightshift` has 1 times, not a start and an end"
    );
    assert_eq!(
        refusal("[recap]\nnightshift = [\"22:00\", \"6:00\"]\n"),
        "`recap` key `nightshift` has time `6:00`, which is not an `HH:MM` time of day"
    );
}

#[test]
fn a_gap_after_the_night_is_refused_with_nightshift_named() {
    assert_eq!(
        refusal("[recap]\nmorning = [\"06:30\", \"12:00\"]\n"),
        "`recap` window `nightshift` ends at 06:00 and `morning` starts at 06:30, \
         so the four windows leave the day uncovered"
    );
}

#[test]
fn pregenerate_takes_nightshift_and_its_refusal_lists_it() {
    assert_eq!(
        parse_config("[recap]\npregenerate = [\"nightshift\"]\n")
            .unwrap()
            .recap
            .pregenerate,
        ["nightshift"]
    );
    assert_eq!(
        refusal("[recap]\npregenerate = [\"nightshfit\"]\n"),
        "`recap` key `pregenerate` names `nightshfit`, which is no window; it takes \
         nightshift, morning, afternoon, evening, today, yesterday, week, last-week"
    );
}
