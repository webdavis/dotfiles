mod tests {
    use super::super::*;
    use crate::runtime_test_support::*;

    #[test]
    fn a_job_waits_while_its_own_child_lives_and_fires_once_that_child_has_gone() {
        // THE TWO HALVES OF THE ONE-CHILD RULE, run in the order the daemon
        // runs them. A seamless breath is issued to still be running when its
        // child exits, so the schedule alone can no longer promise the previous
        // child is gone: `decide` is told whether one is, and it is told the
        // truth only because the reap happens first.
        let state = scratch("daemon-pass-one-child");
        let spool = pns::daemon::spool_dir(&state);
        std::fs::create_dir_all(&spool).expect("the spool");
        let job = pns::daemon::Job {
            id: "lights".to_string(),
            due: 100,
            until: 100_000,
            every: Some(12),
            unless_marker: None,
            // THE HARNESS'S OWN LISTING FLAG: a fired job re-executes THIS
            // binary, which under test is the test binary, and listing its
            // tests exits at once with nothing on either stream.
            args: vec!["--list".to_string()],
        };
        pns::daemon::hand_back(&spool, &job).expect("the record lands");
        let record = spool.join("lights");
        let armed = std::fs::read_to_string(&record).expect("the record is readable");
        // THE RECORD'S IDENTITY, not just its bytes. A wait must never CLAIM,
        // because a claim is a rename out and a write back, and a refresh that
        // landed in between would be overwritten by the copy this daemon was
        // already holding. The inode is what says the file was never replaced.
        let armed_inode = std::os::unix::fs::MetadataExt::ino(
            &std::fs::metadata(&record).expect("the record is there"),
        );

        let mut children = vec![Bounded {
            id: "lights".to_string(),
            child: std::process::Command::new("/bin/sleep")
                .arg("30")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .expect("a child that is still running"),
            expires_at: std::time::Instant::now() + Duration::from_secs(300),
        }];
        let mut reported = std::collections::BTreeSet::new();
        daemon_pass(
            &spool,
            &state,
            Some(200),
            Duration::from_secs(1),
            &mut children,
            &mut reported,
        );
        assert_eq!(
            std::fs::read_to_string(&record).ok().as_deref(),
            Some(armed.as_str()),
            "a job due while its own child was still running fired anyway, so two \
             children were driving one house"
        );
        assert_eq!(children.len(), 1, "and the live child was not reaped");
        assert_eq!(
            std::os::unix::fs::MetadataExt::ino(
                &std::fs::metadata(&record).expect("the record is still there")
            ),
            armed_inode,
            "a waiting job's record was claimed and written back, which is the one \
             write that can lose a refresh a client landed in the meantime"
        );

        // THE CHILD IS GONE NOW, and the occurrence that was held fires on the
        // very next pass rather than being lost.
        let _ = children[0].child.kill();
        let _ = children[0].child.wait();
        daemon_pass(
            &spool,
            &state,
            Some(200),
            Duration::from_secs(1),
            &mut children,
            &mut reported,
        );
        assert_ne!(
            std::fs::read_to_string(&record).ok().as_deref(),
            Some(armed.as_str()),
            "the job never fired once its child had exited, which is a reap that \
             ran after the drain rather than before it"
        );
        for bounded in &mut children {
            let _ = bounded.child.kill();
            let _ = bounded.child.wait();
        }
    }
}
