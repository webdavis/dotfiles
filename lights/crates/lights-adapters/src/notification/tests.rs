use super::*;
use lights_domain::RoomName;
use std::cell::Cell;
mod bounded;
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
    let notifier = PnsNotifier::with_runner(
        Path::new("/owned/home"),
        |command: &mut Command, duration: Duration| {
            assert_eq!(command.get_program(), "/owned/home/.cargo/bin/pns");
            let args = command
                .get_args()
                .map(|s| s.to_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(
                args,
                [
                    "send",
                    "--producer",
                    "lights",
                    "--state",
                    "done",
                    "--project",
                    "Studio",
                    "--detail",
                    "Studio: on",
                    "--scope",
                    "local_only"
                ]
            );
            assert_eq!(duration, Duration::from_secs(2));
            calls.set(calls.get() + 1);
            Ok(ExitStatus::from_raw(0))
        },
    );
    notifier.announce(&action());
    assert_eq!(calls.get(), 1);
}
