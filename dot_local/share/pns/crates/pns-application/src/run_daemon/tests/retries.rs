use super::*;
use std::cell::Cell;

#[test]
fn each_enabled_tick_retries_once_after_spool_work_with_the_sampled_clock() {
    let world = World::new(vec![Ok(true), Ok(false)]);
    let calls = Cell::new(0);
    assert_eq!(
        world.run_with_retry(true, |now| {
            assert_eq!(now, 100);
            let log = world.log.borrow();
            assert_eq!(&log[log.len() - 3..], ["reap", "heartbeat", "entries"]);
            drop(log);
            world.log.borrow_mut().push("retry".into());
            calls.set(calls.get() + 1);
            Ok(())
        }),
        0
    );
    assert_eq!(calls.get(), 29);
    assert_eq!(world.count("reap"), 29);
    assert_eq!(world.count("sleep"), 30);
}

#[test]
fn a_retry_failure_is_reported_and_the_next_tick_still_reaps_and_retries() {
    let world = World::new(vec![Ok(true), Ok(false)]);
    let calls = Cell::new(0);
    assert_eq!(
        world.run_with_retry(true, |_| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Err("owned pending ledger failure".into())
            } else {
                Ok(())
            }
        }),
        0
    );
    assert_eq!(calls.get(), 29);
    assert_eq!(world.count("reap"), 29);
    assert_eq!(
        world.count("err:pns daemon: delivery retry failed: owned pending ledger failure"),
        1
    );
}

#[test]
fn retries_require_successful_enabled_startup_and_a_valid_wall_clock() {
    for (settings, prepare, now, expected) in [
        (vec![Ok(true), Ok(false)], true, Some(100), 29),
        (vec![Ok(false)], true, Some(100), 0),
        (vec![Ok(true)], false, Some(100), 0),
        (vec![Ok(true), Ok(false)], true, None, 0),
    ] {
        let world = World {
            now,
            ..World::new(settings)
        };
        let calls = Cell::new(0);
        assert_eq!(
            world.run_with_retry(prepare, |_| {
                calls.set(calls.get() + 1);
                Ok(())
            }),
            0
        );
        assert_eq!(calls.get(), expected, "prepare={prepare}, now={now:?}");
        if now.is_none() {
            assert_eq!(world.count("reap"), 29);
        }
    }
}
