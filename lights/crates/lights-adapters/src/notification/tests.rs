use super::*;
use lights_domain::RoomName;
use std::{cell::Cell, os::unix::process::ExitStatusExt};
mod lifecycle;

fn action() -> Action {
    Action::PowerSet {
        room: RoomName::new("Studio").unwrap(),
        on: true,
    }
}

#[test]
fn pns_arguments_are_local_only() {
    let calls = Cell::new(0);
    let notifier = PnsNotifier::with_runner(Path::new("/owned/home"), |command: &mut Command| {
        assert_eq!(command.get_program(), "gtimeout");
        let args = command
            .get_args()
            .map(|s| s.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "--foreground",
                "--signal=KILL",
                "2s",
                "/owned/home/.cargo/bin/pns",
                "--agent",
                "lights",
                "--state",
                "done",
                "--project",
                "Studio",
                "--detail",
                "Studio: on",
                "--local-only"
            ]
        );
        calls.set(calls.get() + 1);
        Ok(ExitStatus::from_raw(0))
    });
    notifier.announce(&action());
    assert_eq!(calls.get(), 1);
}
#[test]
fn missing_timeout_monitor_does_not_spawn_pns() {
    let fixture = lifecycle::Fixture::new("missing-monitor");
    let mut notifier = PnsNotifier::new(&fixture.root);
    notifier.pns = fixture.child.clone();
    notifier.monitor = fixture.root.join("absent-monitor");
    notifier.announce(&action());
    assert!(!fixture.ready.exists());
}
