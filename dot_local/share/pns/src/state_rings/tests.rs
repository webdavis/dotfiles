mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_pending_file_left_behind_wide_open_is_narrowed_before_the_rename_publishes_it() {
        // MEASURED: `OpenOptions::mode` applies only when the open CREATES the
        // file, so a pending inode an earlier run left at the umask's mode
        // keeps it, and the rename is what publishes that mode OVER the state
        // file. The pending path carries this process's own id, which is
        // exactly what makes a run interrupted between the open and the rename
        // leave one for the next run of the same pid to reuse.
        let directory =
            std::env::temp_dir().join(format!("pns-publish-mode-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("the scratch directory");
        let published = directory.join("missed-notifications");
        let pending = published.with_extension(format!("new.{}", std::process::id()));
        std::fs::write(&pending, "an interrupted run\n").expect("the pending file");
        // STATED RATHER THAN INHERITED from the umask, so the fixture is the
        // same wide mode on every machine and on a rerun that found its own
        // leftovers.
        std::fs::set_permissions(&pending, std::fs::Permissions::from_mode(0o644))
            .expect("the wide mode");

        publish_state_line(&published, "one line").expect("the publish");

        // THE PUBLISH REALLY RAN, asserted before the mode: a file left from an
        // earlier run already at 0600 would pass the mode assertion alone.
        assert_eq!(
            std::fs::read_to_string(&published).expect("the published file"),
            "one line\n"
        );
        assert_eq!(
            published_mode(&published),
            STATE_FILE_MODE,
            "the reused pending inode published its own wide mode"
        );
    }

    #[test]
    fn a_ring_that_vanished_under_the_append_is_never_republished_over() {
        // THE ONE ERROR THAT IS NOT A DAMAGED RING. Nothing removes one of
        // these files except a claim, and a claim is a RENAME, so a read-back
        // that finds nothing means the line just written travelled inside the
        // claim and is already on its way to the operator. Republishing it
        // would put an already-claimed record back at the path and deliver it
        // a second time.
        //
        // THE LIMIT, stated: this pins the DECISION, not the wiring. Staging a
        // real claim between the append's write and its read-back is a race no
        // test in this tree can make deterministic, and it belongs to the
        // out-of-tree probe.
        assert!(!republish_after(&std::io::Error::from(
            std::io::ErrorKind::NotFound
        )));
        // AND EVERY OTHER REASON STILL HEALS: a ring that cannot be read is a
        // ring that can never be pruned again, which is what the republish is
        // for. These three are exactly what the guarded reader answers with.
        for kind in [
            std::io::ErrorKind::InvalidData,
            std::io::ErrorKind::InvalidInput,
            std::io::ErrorKind::FileTooLarge,
        ] {
            assert!(
                republish_after(&std::io::Error::from(kind)),
                "a ring that answered {kind:?} was left unhealed"
            );
        }
    }
}
