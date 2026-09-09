use super::{SqliteStore, state};
use pns_domain::{
    lamps::config::Behaviour,
    lights::{
        held::Held,
        mute::Muted,
        phase::{HeldEntry, Phase},
        streak::Streak,
        unread::News,
    },
};
#[test]
fn held_lamp_paths_and_phases_round_trip_in_order_and_an_empty_write_clears_them() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    assert!(store.read_held().unwrap().is_empty());
    let held = vec![
        HeldEntry::bare("light/first"),
        HeldEntry {
            path: "grouped_light/second".into(),
            resume: Some(Phase {
                end_unix_ms: u64::MAX,
                landed_on: 255,
                held: Held::UnreadFailure,
            }),
        },
    ];
    store.remember_held(&held).unwrap();
    let reopened = SqliteStore::new(path);
    assert_eq!(reopened.read_held().unwrap(), held);
    reopened.remember_held(&[]).unwrap();
    assert!(reopened.read_held().unwrap().is_empty());
}
#[test]
fn a_failed_held_lamp_replacement_rolls_back_instead_of_losing_the_previous_names() {
    let store = SqliteStore::new(state());
    store
        .remember_held(&[HeldEntry::bare("light/prior")])
        .unwrap();
    assert_eq!(
        store.read_held().unwrap(),
        vec![HeldEntry::bare("light/prior")]
    );
    let connection = store.connect().unwrap();
    connection.execute_batch("CREATE TRIGGER refuse_held BEFORE INSERT ON held_lamps BEGIN SELECT RAISE(ABORT, 'fixture refusal'); END;").unwrap();
    assert!(
        store
            .remember_held(&[HeldEntry::bare("light/new")])
            .is_err()
    );
    assert_eq!(
        store.read_held().unwrap(),
        vec![HeldEntry::bare("light/prior")]
    );
}
#[test]
fn lamp_mutes_keep_place_bytes_and_expiry_order_and_clear_as_one_set() {
    let store = SqliteStore::new(state());
    let entries = vec![
        Muted {
            expiry: 0,
            place: "3F - Master Bedroom".into(),
        },
        Muted {
            expiry: i64::MAX as u64,
            place: "Kitchen".into(),
        },
    ];
    store.write_muted(&entries).unwrap();
    assert_eq!(store.read_muted().unwrap(), entries);
    store.write_muted(&[]).unwrap();
    assert!(store.read_muted().unwrap().is_empty());
}
#[test]
fn lamp_news_merges_both_kinds_without_moving_either_epoch_backwards() {
    let path = state();
    let store = SqliteStore::new(path.clone());
    store.record_news(Behaviour::Blocked, Some(100)).unwrap();
    store.record_news(Behaviour::Done, None).unwrap();
    assert!(
        !path.exists(),
        "irrelevant events must not initialize storage"
    );
    store.record_news(Behaviour::Done, Some(9)).unwrap();
    store.record_news(Behaviour::Failed, Some(11)).unwrap();
    store.record_news(Behaviour::Done, Some(8)).unwrap();
    assert_eq!(
        SqliteStore::new(path).read_news().unwrap(),
        News {
            done_at: Some(9),
            failed_at: Some(11)
        }
    );
}
#[test]
fn the_working_streak_survives_the_exact_grace_edge_and_resets_one_second_later() {
    let store = SqliteStore::new(state());
    assert_eq!(
        store.advance_streak(true, 10).unwrap(),
        Some(Streak {
            since: 10,
            last_seen: 10
        })
    );
    assert_eq!(
        store.advance_streak(true, 20).unwrap(),
        Some(Streak {
            since: 10,
            last_seen: 20
        })
    );
    for now in [139, 140] {
        assert_eq!(
            store.advance_streak(false, now).unwrap(),
            Some(Streak {
                since: 10,
                last_seen: 20
            })
        );
    }
    assert_eq!(store.advance_streak(false, 141).unwrap(), None);
    assert_eq!(
        store.advance_streak(true, 142).unwrap(),
        Some(Streak {
            since: 142,
            last_seen: 142
        })
    );
}
