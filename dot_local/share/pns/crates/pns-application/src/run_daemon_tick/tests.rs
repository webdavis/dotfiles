use super::RunDaemonTick;
use crate::DaemonNotice;
mod fixture;
use fixture::rig;

#[test]
fn an_unreadable_clock_stops_the_drain_but_never_the_child_reap() {
    let (spool, mut children, trace) = rig();
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(None, &mut Default::default(), |_| panic!("no notice"));
    assert_eq!(*trace.borrow(), ["reap"]);
}

#[test]
fn a_wait_is_decided_after_reaping_without_claiming_or_replacing_its_record() {
    let (spool, mut children, trace) = rig();
    children.running = true;
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |_| {
        panic!("a wait says nothing")
    });
    assert_eq!(
        *trace.borrow(),
        ["reap", "heartbeat:100", "entries", "read0"]
    );
}

#[test]
fn a_claimed_refresh_is_read_again_and_put_back_without_firing_the_old_arguments() {
    let (mut spool, mut children, trace) = rig();
    let owned = spool.owned.as_mut().unwrap();
    owned.due = 200;
    owned.args = vec!["refreshed".into()];
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |_| {
        panic!("a put back says nothing")
    });
    assert_eq!(
        *trace.borrow(),
        [
            "reap",
            "heartbeat:100",
            "entries",
            "read0",
            "claim",
            "read1",
            "publish:200:refreshed",
            "release"
        ]
    );
}

#[test]
fn the_repeat_is_durable_and_the_claim_released_before_the_owned_arguments_start() {
    let (mut spool, mut children, trace) = rig();
    spool.owned.as_mut().unwrap().args = vec!["claimed".into()];
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |_| {
        panic!("a successful firing says nothing")
    });
    assert_eq!(
        *trace.borrow(),
        [
            "reap",
            "heartbeat:100",
            "entries",
            "read0",
            "claim",
            "read1",
            "publish:110:claimed",
            "release",
            "start:claimed"
        ]
    );
}

#[test]
fn an_unclaimed_record_is_never_read_again_or_started() {
    let (mut spool, mut children, trace) = rig();
    spool.claim = false;
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |_| {
        panic!("another daemon claimed it")
    });
    assert_eq!(
        *trace.borrow(),
        ["reap", "heartbeat:100", "entries", "read0", "claim"]
    );
}

#[test]
fn an_irregular_entry_is_named_once_and_never_claimed() {
    let (mut spool, mut children, trace) = rig();
    spool.peek = None;
    let mut reported = Default::default();
    let mut notices = Vec::new();
    for _ in 0..2 {
        RunDaemonTick {
            spool: &spool,
            children: &mut children,
        }
        .run(Some(100), &mut reported, |notice| match notice {
            DaemonNotice::Error(line) => notices.push(line),
            _ => panic!("stderr only"),
        });
    }
    assert_eq!(
        notices,
        ["pns daemon: entry0 is not a regular file; left alone and never opened"]
    );
    assert!(!trace.borrow().iter().any(|step| step == "claim"));
}

#[test]
fn an_unusable_claim_is_reported_and_released_without_starting() {
    let (mut spool, mut children, trace) = rig();
    spool.owned = None;
    let mut notices = Vec::new();
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |notice| match notice {
        DaemonNotice::Output(line) => notices.push(line),
        _ => panic!("stdout"),
    });
    assert_eq!(notices, ["pns daemon: dropped `job`: unreadable"]);
    assert_eq!(
        *trace.borrow(),
        [
            "reap",
            "heartbeat:100",
            "entries",
            "read0",
            "claim",
            "read1",
            "release"
        ]
    );
}

#[test]
fn failed_rearm_release_and_spawn_are_all_reported_without_hiding_the_next_attempt() {
    let (mut spool, mut children, trace) = rig();
    spool.hand_back = Err("publish refused".into());
    spool.release = Err("unlink refused".into());
    children.fail = true;
    let mut notices = Vec::new();
    RunDaemonTick {
        spool: &spool,
        children: &mut children,
    }
    .run(Some(100), &mut Default::default(), |notice| match notice {
        DaemonNotice::Error(line) => notices.push(line),
        _ => panic!("stderr"),
    });
    assert_eq!(
        notices,
        [
            "pns daemon: `job` will not repeat (publish refused)",
            "pns daemon: the working file entry1 could not be removed (unlink refused); it is left behind",
            "pns daemon: `job` could not start (spawn refused)"
        ]
    );
    assert_eq!(
        &trace.borrow()[6..],
        ["publish:110:original", "release", "start:original"]
    );
}
