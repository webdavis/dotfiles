use super::*;

#[test]
fn each_store_keeps_its_own_two_consecutive_growth_history() {
    for kind in [QueueKind::Legacy, QueueKind::Pns] {
        let counts = |pending| QueueCounts {
            pending: Some(pending),
            deadletters: Some(0),
            alarm_generation: None,
        };
        let (first, problems) = judge_queue(kind, counts(3), QueueMemory::default());
        assert_eq!(first.count, Some(3));
        assert!(problems.is_empty());
        let (second, problems) = judge_queue(kind, counts(4), first);
        assert_eq!(second.growth_streak, 1);
        assert!(problems.is_empty());
        let (third, problems) = judge_queue(kind, counts(5), second);
        assert_eq!(third.growth_streak, 2);
        assert_eq!(problems.len(), 1);
        let (flat, problems) = judge_queue(kind, counts(5), third);
        assert_eq!(flat.growth_streak, 0);
        assert!(problems.is_empty());
    }
}
#[test]
fn existing_deadletters_and_each_unreadable_counter_page() {
    for kind in [QueueKind::Legacy, QueueKind::Pns] {
        let (_, problems) = judge_queue(
            kind,
            QueueCounts {
                pending: Some(0),
                deadletters: Some(1),
                alarm_generation: None,
            },
            QueueMemory::default(),
        );
        assert_eq!(problems.len(), 1);
        let (next, problems) = judge_queue(
            kind,
            QueueCounts {
                pending: None,
                deadletters: None,
                alarm_generation: None,
            },
            QueueMemory {
                count: Some(5),
                growth_streak: 1,
                deadletters: None,
            },
        );
        assert_eq!(problems.len(), 2);
        assert_eq!(next, QueueMemory::default());
    }
}
fn with_deadletters(deadletters: u64) -> QueueCounts {
    QueueCounts {
        pending: Some(0),
        deadletters: Some(deadletters),
        alarm_generation: None,
    }
}
#[test]
fn a_retained_dead_letter_pages_once_and_then_holds_its_peace() {
    for kind in [QueueKind::Legacy, QueueKind::Pns] {
        let (first, problems) = judge_queue(kind, with_deadletters(2), QueueMemory::default());
        assert_eq!(problems.len(), 1, "the first sighting has to be reported");
        assert_eq!(first.deadletters, Some(2));
        let (second, problems) = judge_queue(kind, with_deadletters(2), first);
        assert!(
            problems.is_empty(),
            "pns retains dead letters forever, so an unchanged count is old news: {problems:?}"
        );
        assert_eq!(second.deadletters, Some(2));
    }
}
#[test]
fn a_further_dead_letter_pages_and_names_how_many_are_new() {
    let prior = QueueMemory {
        deadletters: Some(2),
        ..QueueMemory::default()
    };
    let (next, problems) = judge_queue(QueueKind::Pns, with_deadletters(5), prior);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains('3'), "no delta named: {}", problems[0]);
    assert_eq!(next.deadletters, Some(5));
}
#[test]
fn a_purge_lowers_the_baseline_silently_so_the_next_one_pages_again() {
    let prior = QueueMemory {
        deadletters: Some(9),
        ..QueueMemory::default()
    };
    let (purged, problems) = judge_queue(QueueKind::Legacy, with_deadletters(0), prior);
    assert!(problems.is_empty(), "a purge is good news: {problems:?}");
    assert_eq!(purged.deadletters, Some(0));
    let (_, problems) = judge_queue(QueueKind::Legacy, with_deadletters(1), purged);
    assert_eq!(problems.len(), 1);
}
#[test]
fn an_unreadable_pending_count_keeps_the_dead_letter_baseline() {
    let prior = QueueMemory {
        count: Some(4),
        growth_streak: 1,
        deadletters: Some(2),
    };
    let (next, _) = judge_queue(
        QueueKind::Pns,
        QueueCounts {
            pending: None,
            deadletters: Some(2),
            alarm_generation: None,
        },
        prior,
    );
    assert_eq!(next.count, None);
    assert_eq!(next.growth_streak, 0);
    assert_eq!(
        next.deadletters,
        Some(2),
        "forgetting the baseline re-pages the same dead letters next tick"
    );
}
#[test]
fn legacy_growth_survives_the_first_rust_tick_and_cannot_overflow() {
    let (next, problems) = judge_queue(
        QueueKind::Legacy,
        QueueCounts {
            pending: Some(8),
            deadletters: Some(0),
            alarm_generation: None,
        },
        QueueMemory {
            count: Some(7),
            growth_streak: 1,
            deadletters: None,
        },
    );
    assert_eq!(next.growth_streak, 2);
    assert_eq!(problems.len(), 1);
    let (next, problems) = judge_queue(
        QueueKind::Pns,
        QueueCounts {
            pending: Some(8),
            deadletters: Some(0),
            alarm_generation: None,
        },
        QueueMemory {
            count: Some(7),
            growth_streak: u64::MAX,
            deadletters: None,
        },
    );
    assert_eq!(next.growth_streak, u64::MAX);
    assert_eq!(problems.len(), 1);
}
