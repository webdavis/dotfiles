mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn a_lease_is_renewed_only_while_it_exists_and_swept_once_it_times_out() {
        // THE WIRING, not the rule. `loop_running` is pure and total and reads
        // no directory, so a lease list invented at the call site leaves every
        // one of its unit tests green while the lamp never arms by hand. The
        // renewal is the half that matters most: it must never CREATE a lease,
        // or every event from every pane would take one.
        const TIMEOUT: u64 = 3_900;
        let state = scratch("loop-lease");
        let marker =
            pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id names a lease");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");

        renew_loop_lease(&state, "wW:p21", Some(1_000));
        assert!(
            !marker.exists(),
            "a pane with no lease is not given one by its own traffic"
        );

        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        renew_loop_lease(&state, "wW:p21", Some(2_000));
        assert_eq!(
            sweep_leases(&state, 2_000, TIMEOUT),
            vec![2_000],
            "the pane's own traffic moved the lease forward"
        );
        assert_eq!(
            sweep_leases(&state, 2_000 + TIMEOUT, TIMEOUT),
            vec![2_000],
            "exactly at the timeout it is still live: both edges closed"
        );
        assert_eq!(
            sweep_leases(&state, 2_000 + TIMEOUT + 1, TIMEOUT),
            Vec::<u64>::new(),
            "and one second past it, an abandoned lease is gone"
        );
        assert!(
            !marker.exists(),
            "swept on the way through, because nothing else would ever remove it"
        );
        // AN UNREADABLE LEASE IS SWEPT TOO: nothing can age out a file whose
        // epoch cannot be read.
        std::fs::write(&marker, "not an epoch\n").expect("a garbled lease");
        assert_eq!(sweep_leases(&state, 2_000, TIMEOUT), Vec::<u64>::new());
        assert!(!marker.exists());
    }

    #[test]
    fn a_renewal_writes_through_the_lease_it_found_rather_than_publishing_a_new_one() {
        // A LEASE `pns loop end` REMOVED MUST STAY REMOVED. A look followed by
        // a publish is two moments: an end landing between them is undone by
        // the rename, and the lamp then breathes for a whole timeout over work
        // that finished. Writing through a handle opened on the EXISTING file
        // closes that window, because an unlink after the open sends the bytes
        // to an inode nobody can reach.
        //
        // THE INODE IS WHAT PROVES IT, and it is the only observable difference:
        // a publish-by-rename leaves a different file at the same path.
        let state = scratch("lease-renew-in-place");
        let marker = pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");
        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        let before = std::fs::metadata(&marker).expect("the lease").ino();

        renew_loop_lease(&state, "wW:p21", Some(1_700_000_002));

        assert_eq!(
            std::fs::metadata(&marker).expect("the lease").ino(),
            before,
            "the renewal published a NEW file over the lease, so an end landing \
             between the look and the rename is undone by it"
        );
        assert_eq!(
            std::fs::read_to_string(&marker).expect("the lease"),
            "1700000002\n",
            "and the epoch really moved: the file is rewritten, not merely kept"
        );
        // AND A SHORTER EPOCH LEAVES NO TAIL of the longer one behind it, which
        // is what the truncation after the write is for.
        renew_loop_lease(&state, "wW:p21", Some(9));
        assert_eq!(std::fs::read_to_string(&marker).expect("the lease"), "9\n");
    }

    #[test]
    fn a_lease_that_could_not_be_given_back_is_reported_rather_than_called_a_success() {
        // THE WORST OUTCOME THIS VERB HAS: telling the operator a loop has
        // ended while its lease is still on disk. The lamp is a liveness signal,
        // so it goes on breathing for the whole timeout with nothing behind it,
        // and they have been told the opposite.
        let state = scratch("lease-end-refused");
        std::fs::create_dir_all(pns::lights::lease_dir(&state)).expect("the lease directory");
        assert_eq!(
            end_lease(&state, "wW:p21"),
            Ok(()),
            "a machine that never began is a removal of a file that is not there"
        );
        let marker = pns::lights::lease_marker(&state, "wW:p21").expect("herdr's own id");
        std::fs::write(&marker, "1000\n").expect("a lease taken by hand");
        assert_eq!(end_lease(&state, "wW:p21"), Ok(()));
        assert!(!marker.exists(), "and the lease is really gone");

        std::fs::create_dir(&marker).expect("a directory standing where the lease goes");
        let refused = end_lease(&state, "wW:p21").expect_err("a lease that will not be removed");
        assert!(
            refused.contains("the lease could not be given back"),
            "{refused}"
        );
    }
}
