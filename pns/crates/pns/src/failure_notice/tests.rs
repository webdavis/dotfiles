use super::*;
use pns_domain::retry::DeliveryOutcome;

fn stored(id: u64, failed_at: u64, retries: u64, deadlettered: bool) -> StoredFailure {
    StoredFailure {
        id,
        destination: "hermes".into(),
        route: "testpath".into(),
        agent: "posture".into(),
        state: "failed".into(),
        outcome: DeliveryOutcome::Status(404),
        failed_at,
        retries,
        deadlettered,
    }
}

const PASS_BEGAN: u64 = 1_000;

/// A leg that failed BEFORE this pass began was already announced by the pass
/// that recorded it. Re-announcing it every pass is how a banner becomes noise.
#[test]
fn a_failure_from_an_earlier_pass_is_not_announced_again() {
    let failures = [stored(1, PASS_BEGAN - 1, 0, false)];
    assert!(to_announce(&failures, PASS_BEGAN).is_empty());
}

/// The first failure of this pass is the news that something broke, and for a
/// permanent refusal it is the only moment there will be.
#[test]
fn a_first_failure_in_this_pass_is_announced() {
    let failures = [stored(7, PASS_BEGAN, 0, false)];
    assert_eq!(
        to_announce(&failures, PASS_BEGAN)
            .iter()
            .map(|stored| stored.id)
            .collect::<Vec<_>>(),
        vec![7]
    );
}

/// A retry in the middle says nothing the first failure did not.
#[test]
fn a_middle_retry_in_this_pass_stays_quiet() {
    let failures = [stored(7, PASS_BEGAN + 5, 4, false)];
    assert!(to_announce(&failures, PASS_BEGAN).is_empty());
}

/// Giving up is news: the page is lost rather than late.
#[test]
fn the_dead_letter_speaks_even_after_many_quiet_retries() {
    let failures = [stored(7, PASS_BEGAN + 5, 19, true)];
    assert_eq!(to_announce(&failures, PASS_BEGAN).len(), 1);
}

/// The listing is newest-first over every failing leg, so a pass reads rows it
/// did not write. Both filters are needed, and this pins that neither one alone
/// would do.
#[test]
fn a_pass_speaks_only_for_the_rows_it_recorded() {
    let failures = [
        stored(9, PASS_BEGAN + 1, 0, false),
        stored(8, PASS_BEGAN - 1, 0, false),
        stored(7, PASS_BEGAN + 1, 3, false),
    ];
    assert_eq!(
        to_announce(&failures, PASS_BEGAN)
            .iter()
            .map(|stored| stored.id)
            .collect::<Vec<_>>(),
        vec![9]
    );
}

fn composed(destination: &str) -> Failure {
    crate::command_failures::compose(&StoredFailure {
        destination: destination.into(),
        ..stored(1, PASS_BEGAN, 0, false)
    })
}

/// NEVER THROUGH THE DESTINATION THAT FAILED. A card about a push that was
/// refused goes out through the destination that just refused one, so it
/// arrives nowhere and the operator learns nothing.
#[test]
fn a_mobile_failure_pushes_no_card_about_itself() {
    assert_eq!(card_surface(&composed("mobile"), true), None);
}

/// hermes carries the full form, so a hermes failure means there is nothing in
/// Discord to point the phone reader at. The card still goes: the phone is not
/// what failed.
#[test]
fn a_hermes_failure_cards_the_phone_and_says_discord_is_empty() {
    assert_eq!(
        card_surface(&composed("hermes"), true),
        Some(NotificationSurface::Phone {
            serve: true,
            hermes_failed: true,
        })
    );
}

/// `serve` rides through untouched: it is the switch that decides which pointer
/// the fix line carries, and this module decides nothing about the wording.
#[test]
fn the_serve_switch_reaches_the_card_as_written() {
    for serve in [true, false] {
        assert_eq!(
            card_surface(&composed("hermes"), serve),
            Some(NotificationSurface::Phone {
                serve,
                hermes_failed: true,
            })
        );
    }
}
