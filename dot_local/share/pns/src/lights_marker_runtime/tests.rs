mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn the_first_tick_sweeps_the_state_the_old_names_held() {
        // THE DEPLOY TRANSITION: delete, dark direction, once. Files under the
        // old names would otherwise sit unread forever, and the old held-glow
        // record names lamps only the binary that is gone knew how to put out.
        let state = scratch("legacy-sweep");
        std::fs::write(state.join("lights-glow"), "light/l9\n").expect("the old held record");
        std::fs::write(state.join("lights-working-since"), "1000\n").expect("the old streak");
        std::fs::create_dir_all(state.join("lights-needs")).expect("the old needs directory");
        std::fs::write(state.join("lights-needs").join("s1"), "1000\n").expect("an old wait");
        sweep_legacy_state(&state);
        assert!(
            !state.join("lights-glow").exists()
                && !state.join("lights-working-since").exists()
                && !state.join("lights-needs").exists(),
            "every old name is gone, contents and all"
        );
    }
    /// One shell's marker planted by hand: the pid it is named for, and the
    /// second its command started.
    fn plant_shell_marker(state: &std::path::Path, pid: &str, body: &str) -> PathBuf {
        let shell = state.join(LIGHTS_SHELL_DIR);
        std::fs::create_dir_all(&shell).expect("the shell marker directory");
        let path = shell.join(pid);
        std::fs::write(&path, body).expect("the shell marker");
        path
    }

    #[test]
    fn the_shell_reading_is_the_oldest_marker_a_live_shell_is_holding() {
        // THE LONGEST-RUNNING COMMAND IS WHAT THE THRESHOLDS MEASURE. One
        // shell per pane means several markers at once, and the freshest of
        // them would restart the breathe clock every time any pane ran
        // anything, so a build running for an hour beside a prompt someone
        // keeps typing at would never reach a threshold measured in minutes.
        //
        // TWO KINDS OF LIVE SHELL, because `kill(pid, 0)` has two ways of
        // saying the process is there: this test's own process answers
        // success, and pid 1 is launchd, which this user may not signal and
        // which answers EPERM. Only ESRCH is gone.
        let state = scratch("lights-shell-oldest");
        plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");
        plant_shell_marker(&state, "1", "1000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(1000),
            "the reading must be the oldest live marker, not the newest and \
             not whichever the directory happened to list first"
        );
    }

    #[test]
    fn a_marker_whose_shell_is_gone_is_swept_and_never_read() {
        // A SHELL KILLED MID-COMMAND is the case the pid in the name exists
        // for. Nothing else would ever remove that file: its own precmd never
        // runs again and its EXIT trap never fired, so without this sweep it
        // is both a lamp breathing forever about a command nobody is running
        // and one file per killed terminal for the life of the machine.
        let state = scratch("lights-shell-dead-pid");
        let dead = a_reaped_pid().to_string();
        let dead_marker = plant_shell_marker(&state, &dead, "1000\n");
        plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(2000),
            "a dead shell's epoch was still being read as work in progress"
        );
        assert!(
            !dead_marker.exists(),
            "and the file it left behind is gone: nothing else ever collects it"
        );
    }

    #[test]
    fn a_name_that_is_not_a_shell_pid_is_swept() {
        // Nothing this crate or the bashrc writes lands here under a name that
        // is not a pid, so anything else is litter no liveness test can ever
        // age out. A NON-POSITIVE NUMBER IS LITTER TOO, and it matters more
        // than it looks: `kill()` reads 0 as this process's own group and -1 as
        // every process the user owns, so a hand-planted `0` or `-1` must never
        // reach the liveness test looking like a pid.
        let state = scratch("lights-shell-bad-name");
        let junk = plant_shell_marker(&state, "not-a-pid", "1000\n");
        let zero = plant_shell_marker(&state, "0", "1000\n");
        let live = plant_shell_marker(&state, &std::process::id().to_string(), "2000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(2000),
            "only a marker a live shell is named by may feed the reading"
        );
        assert!(
            !junk.exists(),
            "the unparseable name was left to accumulate"
        );
        assert!(!zero.exists(), "a non-positive pid was left to accumulate");
        assert!(
            live.exists(),
            "and the sweep took the live shell's marker with it, which would \
             darken the lamp under every build"
        );
    }

    #[test]
    fn a_live_shell_whose_marker_holds_no_epoch_yet_is_left_alone() {
        // THE WRITE IS A TRUNCATING REDIRECT. `printf ... >"$marker"` empties
        // the file at open and fills it a moment later, so a tick landing in
        // that window reads an empty file for a command that is genuinely
        // starting. Unlinking it there wins the race against the write, which
        // then fills a file nothing will ever look at, and the build runs to
        // completion with no marker at all: exactly the dark lamp this whole
        // slice exists to fix. The pid is what collects the file when that
        // shell ends, so nothing accumulates by leaving it.
        let state = scratch("lights-shell-mid-write");
        let starting = plant_shell_marker(&state, &std::process::id().to_string(), "");
        plant_shell_marker(&state, "1", "1000\n");

        assert_eq!(
            sweep_shell_markers(&state),
            Some(1000),
            "an epoch that cannot be read is not an epoch: it must not become \
             a reading of its own"
        );
        assert!(
            starting.exists(),
            "a live shell's marker was unlinked out from under its own write"
        );
    }

    #[test]
    fn no_directory_and_an_empty_one_both_read_as_nothing() {
        // A MACHINE WHOSE SHELL NEVER PUBLISHED is the ordinary case on a host
        // that has not applied this bashrc yet, and it must read as no shell
        // work rather than as an error or a zero epoch: a zero would be a
        // command that started in 1970 and would pass every threshold there is.
        let state = scratch("lights-shell-empty");
        assert_eq!(
            sweep_shell_markers(&state),
            None,
            "a state directory with no shell directory in it read as work"
        );

        std::fs::create_dir_all(state.join(LIGHTS_SHELL_DIR)).expect("the shell directory");
        assert_eq!(
            sweep_shell_markers(&state),
            None,
            "an empty shell directory read as work"
        );
    }

    #[test]
    fn the_ticks_blocked_reading_takes_its_backstop_from_the_config_on_both_halves() {
        // THE TICK COMPOSES TWO READERS OF THE SAME BOUND, the sweep that
        // deletes an aged marker and the aggregate that lights the lamp, and
        // each is handed the knob separately. A knob past every number this
        // bound was ever hardcoded to, and a wait older than all of them but
        // inside it: a reader that kept an old constant on EITHER half puts
        // the lamp out here.
        const GIVE_UP_AFTER_SECS: u64 = 100_000;
        let state = scratch("blocked-knob-tick");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
        std::fs::create_dir_all(marker.parent().expect("the wait directory"))
            .expect("the wait directory");
        std::fs::write(&marker, "1000\n").expect("a wait in progress");
        // THROUGH THE PARSER, not a field poked on a default: the knob the
        // operator writes is the one the tick must read.
        let config = pns::config::parse_config(&format!(
            "[lights.blocked]\ngive_up_after_secs = {GIVE_UP_AFTER_SECS}\n"
        ))
        .expect("a config stating the knob");
        let lights = config.lights.as_deref().expect("the lights table");

        assert!(
            blocked_lamp(&state, lights, 1_000 + 90_000),
            "a day-old question inside the configured backstop still holds the lamp"
        );
        assert!(
            !blocked_lamp(&state, lights, 1_000 + GIVE_UP_AFTER_SECS + 1),
            "and one second past the backstop the lamp is given back"
        );
        assert!(
            !marker.exists(),
            "by the sweep, which read the same knob and removed the marker"
        );
    }

    #[test]
    fn a_wait_nobody_has_answered_still_holds_its_lamp_until_the_configured_backstop() {
        // THE LOCK SAYS "CONTINUOUS UNTIL THE OPERATOR ANSWERS", and half an
        // hour was not that: a question asked while they were at lunch went
        // dark before they came back, with nothing anywhere to say it had. What
        // is left is an ABANDONED-SESSION BACKSTOP and nothing else, so the
        // lamp survives every absence the knob names.
        //
        // A KNOB THAT IS NOT THE SHIPPED DEFAULT, so a `sweep_blocked` that
        // silently kept an old hardcoded number instead of reading the
        // configured one would still be caught here.
        const GIVE_UP_AFTER_SECS: u64 = 3_600;

        let state = scratch("blocked-bound");
        let marker = pns::lights::blocked_marker(&state, "s1").expect("a usable session id");
        std::fs::create_dir_all(marker.parent().expect("the wait directory"))
            .expect("the wait directory");
        std::fs::write(&marker, "1000\n").expect("a wait in progress");

        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS - 1, GIVE_UP_AFTER_SECS),
            vec![1_000],
            "a question just short of the knob is still a question nobody has answered"
        );
        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS, GIVE_UP_AFTER_SECS),
            vec![1_000],
            "exactly at the backstop it is still live: the bound is closed"
        );
        assert_eq!(
            sweep_blocked(&state, 1_000 + GIVE_UP_AFTER_SECS + 1, GIVE_UP_AFTER_SECS),
            Vec::<u64>::new(),
            "and one second past it the abandoned session gives the bulb back"
        );
        assert!(!marker.exists(), "swept on the way through");
    }

    #[test]
    fn the_sweep_leaves_a_marker_that_is_mid_publish_alone() {
        // `publish_state_line` writes `<name>.new.<pid>` INTO THIS DIRECTORY
        // and renames it over the marker, so a pending file is an ordinary
        // entry the sweep walks. Between the open and the rename there is no
        // epoch in it to read, and an unreadable-means-delete rule unlinks it
        // there: the racing rename then publishes nothing and the wait is lost
        // with the agent still waiting on the operator.
        let state = scratch("sweep-skips-pending");
        let needs = pns::lights::blocked_dir(&state);
        std::fs::create_dir_all(&needs).expect("the needs directory");
        std::fs::write(needs.join("s1"), "1000\n").expect("a live wait");
        let pending = needs.join(format!("s2.new.{}", std::process::id()));
        std::fs::write(&pending, "").expect("a marker caught mid-publish");
        std::fs::write(needs.join("s3"), "not an epoch\n").expect("an unreadable marker");

        assert_eq!(
            sweep_blocked(&state, 1000, 3_600),
            vec![1000],
            "the live wait is still what the sweep answers with"
        );
        assert!(
            pending.exists(),
            "and the pending file is left for its own rename to publish"
        );
        assert!(
            !needs.join("s3").exists(),
            "while a marker that really is unreadable is still swept: nothing \
             else ages out a file whose epoch cannot be read"
        );
    }

    #[test]
    fn a_pending_file_whose_run_is_gone_is_collected_and_a_marker_that_spells_it_is_swept() {
        // TWO HALVES OF ONE COLLISION. A session id and a pane id are opaque
        // words from another program, and both alphabets admit a dot, so a name
        // matched on the bare `.new.` put a real marker beyond every sweep: it
        // aged out never and its lamp could not be released. The same match let
        // a publish whose run had DIED sit in the directory forever, which is
        // the unbounded growth the sweep exists to prevent, through a door it
        // opened itself.
        let state = scratch("sweep-pending-collection");
        let leases = pns::lights::lease_dir(&state);
        std::fs::create_dir_all(&leases).expect("the lease directory");
        let spelled = leases.join("a.new.b");
        std::fs::write(&spelled, "1000\n").expect("a pane whose own id spells the suffix");
        let abandoned = leases.join(format!("s2.new.{}", a_reaped_pid()));
        std::fs::write(&abandoned, "").expect("a publish whose run died");
        let in_flight = leases.join(format!("s3.new.{}", std::process::id()));
        std::fs::write(&in_flight, "").expect("a publish still in flight");

        assert_eq!(
            sweep_markers(&leases, 100_000, 60),
            Vec::<u64>::new(),
            "the expired marker is not answered with"
        );
        assert!(
            !spelled.exists(),
            "a marker whose name spells the pending suffix was invisible to the sweep"
        );
        assert!(
            !abandoned.exists(),
            "a publish whose own run is gone is litter nothing else collects"
        );
        assert!(
            in_flight.exists(),
            "while a publish still in flight is left for its own rename"
        );
    }

    #[test]
    fn a_sweep_takes_a_marker_before_removing_it_and_leaves_no_working_file_behind() {
        // OWNED BY RENAME, NEVER READ-THEN-UNLINK. Concurrent unlink does not
        // arbitrate on this filesystem: it reports success to every caller, so a
        // sweep that read an expired epoch and then unlinked could remove a
        // FRESH marker a racing event published in between, and both runs would
        // believe they had removed the old one.
        //
        // WHAT A SINGLE-THREADED TEST CAN PIN is the shape either way: the
        // expired marker really goes, the live one is untouched, and no working
        // file is left in the directory. The interleaving itself is a race no
        // test in this tree can stage.
        let state = scratch("sweep-owns-by-rename");
        let leases = pns::lights::lease_dir(&state);
        std::fs::create_dir_all(&leases).expect("the lease directory");
        std::fs::write(leases.join("live"), "1000\n").expect("a live lease");
        std::fs::write(leases.join("expired"), "10\n").expect("an expired lease");
        let live_inode = std::fs::metadata(leases.join("live"))
            .expect("the live lease")
            .ino();

        assert_eq!(sweep_markers(&leases, 1_000, 60), vec![1_000]);

        assert!(!leases.join("expired").exists(), "the expired lease goes");
        assert_eq!(
            std::fs::metadata(leases.join("live"))
                .expect("the live lease")
                .ino(),
            live_inode,
            "and the live one is not even renamed: the ordinary tick moves nothing"
        );
        let left: Vec<String> = std::fs::read_dir(&leases)
            .expect("the lease directory")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["live".to_string()], "a claim was left behind");
    }
}
