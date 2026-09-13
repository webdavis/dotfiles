use super::*;

#[test]
fn each_store_keeps_its_own_two_consecutive_growth_history() {
    for kind in [QueueKind::Legacy, QueueKind::Pns] {
        let counts = |pending| QueueCounts {
            pending: Some(pending),
            deadletters: Some(0),
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
            },
            QueueMemory::default(),
        );
        assert_eq!(problems.len(), 1);
        let (next, problems) = judge_queue(
            kind,
            QueueCounts {
                pending: None,
                deadletters: None,
            },
            QueueMemory {
                count: Some(5),
                growth_streak: 1,
            },
        );
        assert_eq!(problems.len(), 2);
        assert_eq!(next, QueueMemory::default());
    }
}
#[test]
fn legacy_growth_survives_the_first_rust_tick_and_cannot_overflow() {
    let (next, problems) = judge_queue(
        QueueKind::Legacy,
        QueueCounts {
            pending: Some(8),
            deadletters: Some(0),
        },
        QueueMemory {
            count: Some(7),
            growth_streak: 1,
        },
    );
    assert_eq!(next.growth_streak, 2);
    assert_eq!(problems.len(), 1);
    let (next, problems) = judge_queue(
        QueueKind::Pns,
        QueueCounts {
            pending: Some(8),
            deadletters: Some(0),
        },
        QueueMemory {
            count: Some(7),
            growth_streak: u64::MAX,
        },
    );
    assert_eq!(next.growth_streak, u64::MAX);
    assert_eq!(problems.len(), 1);
}
