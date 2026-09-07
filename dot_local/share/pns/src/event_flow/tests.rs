mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::cell::RefCell;

    /// Restores an environment variable when the test that set it ends, so a
    /// panicking assertion cannot leave a scratch `HOME` behind for the rest
    /// of the binary.
    struct Restored(&'static str, Option<String>);

    impl Drop for Restored {
        fn drop(&mut self) {
            match &self.1 {
                Some(had) => unsafe { std::env::set_var(self.0, had) },
                None => unsafe { std::env::remove_var(self.0) },
            }
        }
    }

    fn set_env(name: &'static str, value: &std::path::Path) -> Restored {
        let had = std::env::var(name).ok();
        unsafe { std::env::set_var(name, value) };
        Restored(name, had)
    }

    #[test]
    fn the_pulse_is_handed_the_reading_taken_before_the_channels_ran() {
        // THE SNAPSHOT CONTRACT, end to end, which is the one thing a test
        // built out of hand calls to `presence_snapshot` cannot pin: it takes
        // the reading at whatever moment the test chooses rather than the one
        // the event path chooses. Here the event path chooses, and a channel
        // republishes the reading WHILE the legs dispatch.
        //
        // WHAT MAKES IT RED is restoring the LAZY read in
        // `system::with_presence_path` and moving construction down to the
        // pulse: Kitchen received where the decision saw the study. Moving
        // construction ALONE leaves this green, and correctly so, because the
        // eager read already fixed the line before any leg ran. This pins the
        // contract, not the statement order that currently delivers it.
        let home = scratch("pulse-sink-ordering");
        let line = home.join("presence");
        let now = now_secs().expect("a clock");
        std::fs::write(&line, format!("{now} {now} 1 3F - Studio\n")).expect("a reading");
        std::fs::create_dir_all(home.join(".config/pns")).expect("a config directory");
        std::fs::write(
            home.join(".config/pns/config.toml"),
            "[lights]\n\
             [plugins.macos-banner]\n\
             enabled = true\n\
             [plugins.mobile]\n\
             enabled = true\n\
             type = \"moshi\"\n\
             [plugins.presence]\n\
             enabled = true\n\
             type = \"hue\"\n\
             rooms = [\"3F - Studio\", \"2F - Kitchen\"]\n\
             desk_room = \"3F - Studio\"\n",
        )
        .expect("a config");
        // The daemon's poll, standing in as the one thing that runs between
        // the decision and the pulse: a channel handed this event to deliver.
        let channels = home.join("channels");
        std::fs::create_dir_all(&channels).expect("a channels directory");
        // BOTH LOCAL CHANNELS CARRY IT, because which one this event plans is
        // a fact about the machine the suite runs on: the banner is the leg at
        // an unlocked desk and the card is the leg anywhere else, and exactly
        // one of them fires.
        for channel in ["macos-banner", "mobile"] {
            let republish = channels.join(format!("{channel}.sh"));
            std::fs::write(
                &republish,
                format!(
                    "#!/bin/sh\nprintf '%s' '{now} {now} 1 2F - Kitchen' > '{}'\n",
                    line.display()
                ),
            )
            .expect("a channel");
            std::fs::set_permissions(&republish, std::fs::Permissions::from_mode(0o755))
                .expect("an executable channel");
        }
        let _home_var = set_env("HOME", &home);
        let _state_var = set_env("PNS_STATE_DIR", &home.join("state"));
        let _channels_var = set_env("PNS_CHANNELS_DIR", &channels);
        let probes = system_probes().with_presence_path(line.to_string_lossy().into_owned());
        let handed: RefCell<Option<pns::presence_policy::Snapshot>> = RefCell::new(None);
        run_event_pulsing(
            &pns::args::EventArgs {
                agent: "claude".to_string(),
                state: "done".to_string(),
                project: "pns".to_string(),
                long_running: true,
                ..Default::default()
            },
            &probes,
            &HookPayload::default(),
            Attempt::First,
            &|_, _, _, snapshot| *handed.borrow_mut() = snapshot.cloned(),
        );
        assert_eq!(
            std::fs::read_to_string(&line).expect("the channel ran"),
            format!("{now} {now} 1 2F - Kitchen"),
            "the channel republished the reading while the legs dispatched"
        );
        assert_eq!(
            handed.borrow().as_ref().map(|snapshot| &snapshot.status),
            Some(&pns::presence::PresenceStatus::Room {
                room: "3F - Studio".to_string(),
                age_secs: 0,
            }),
            "the pulse is handed the reading the decision saw, never the later one"
        );
    }
}
