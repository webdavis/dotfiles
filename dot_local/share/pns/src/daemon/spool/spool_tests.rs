//! The spool's own transactions against a real directory: publishing,
//! claiming, handing back, and what a hostile directory costs.

use super::{
    Job, Peeked, Startup, claim, hand_back, job_count, marker_dir, marker_exists, parse, peek,
    prepare_spool, publish_job, render, spool_dir, validate_registration,
};
use std::path::{Path, PathBuf};

const NOW: u64 = 1_700_000_000;

/// A private directory per test, removed on every exit path including a
/// panic, in `Sandbox`'s own shape.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("pns-daemon-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("a scratch directory");
        Scratch { root }
    }

    fn spool(&self) -> PathBuf {
        let spool = spool_dir(&self.root);
        std::fs::create_dir_all(&spool).expect("a spool");
        spool
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn job(id: &str, due: u64) -> Job {
    Job {
        id: id.to_string(),
        due,
        until: due + 300,
        every: Some(30),
        unless_marker: None,
        args: vec!["--agent".to_string(), "pns".to_string()],
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("the record")
}

/// A REFRESH PUBLISHED WHILE THE JOB IS CLAIMED SURVIVES THE DAEMON'S
/// RE-ARM.
///
/// The daemon computes a repeat's next occurrence from the record it took;
/// a client registering the same id in that window has published a NEWER
/// signal. An overwriting rename would replace the client's due, lease and
/// argv with the daemon's older reading, which is the id-is-the-filename
/// refresh guarantee failing in the one direction nobody can observe.
#[test]
fn a_refresh_published_while_a_job_is_claimed_survives_the_daemons_re_arm() {
    let scratch = Scratch::new("refresh-beats-rearm");
    let spool = scratch.spool();
    let refreshed = Job {
        args: vec!["--agent".to_string(), "refreshed".to_string()],
        ..job("upkeep", NOW + 5)
    };
    publish_job(&spool, &refreshed).expect("the client's refresh");

    let stale = job("upkeep", NOW);
    assert!(
        !hand_back(&spool, &stale).expect("the re-arm"),
        "the daemon's re-arm must lose to a record already at the id"
    );
    assert_eq!(
        parse(read(&spool.join("upkeep")).trim_end()),
        Ok(refreshed),
        "the client's refresh is what stayed"
    );

    // THE UNMUTATED CONTROL: with the id free, the same call lands.
    let free = job("other", NOW);
    assert!(
        hand_back(&spool, &free).expect("a free id"),
        "a free id must take the daemon's write"
    );
    assert_eq!(parse(read(&spool.join("other")).trim_end()), Ok(free));
}

/// A REGISTRATION LANDING WHILE THE OLD RECORD IS CLAIMED IS NOT DELETED BY
/// THE CLAIM CLEANUP.
///
/// The claim is a RENAME, so the daemon's cleanup removes the working name
/// it holds and never the id. A cleanup that unlinked the id instead would
/// throw away the registration that arrived while the old occurrence ran.
#[test]
fn a_registration_landing_while_the_old_record_is_claimed_is_not_deleted_by_the_cleanup() {
    let scratch = Scratch::new("registration-survives-cleanup");
    let spool = scratch.spool();
    let old = job("nag", NOW);
    publish_job(&spool, &old).expect("the old record");

    let held = claim(&spool.join("nag")).expect("the claim");
    assert!(
        !spool.join("nag").exists(),
        "a claim takes the name with it"
    );

    // The client registers again while the daemon holds the old record.
    let fresh = Job {
        args: vec!["--agent".to_string(), "fresh".to_string()],
        ..job("nag", NOW + 60)
    };
    publish_job(&spool, &fresh).expect("the new registration");

    std::fs::remove_file(&held).expect("the cleanup");
    assert_eq!(
        parse(read(&spool.join("nag")).trim_end()),
        Ok(fresh),
        "the registration that arrived during the claim is what survived"
    );
}

/// AN ARGV THAT PASSES EVERY FIELD BOUND AND STILL RENDERS PAST THE RECORD
/// CAP IS REFUSED AT REGISTRATION.
///
/// The bound on `args` counts the bytes handed in; the record carries them
/// JSON-ESCAPED, so one control character becomes six bytes. Accepted, this
/// wrote a file the daemon could only ever drop as unparseable: a schedule
/// that reported success and could never run.
#[test]
fn an_argv_that_renders_past_the_record_cap_is_refused_by_name() {
    let control_characters = Job {
        args: vec!["\u{1}".repeat(4096)],
        ..job("oversized", NOW)
    };
    let refusal = validate_registration(&control_characters, NOW)
        .expect_err("a record past the cap must be refused");
    assert!(
        refusal.contains("rendered record") && refusal.contains("8192"),
        "the refusal must name the cap it broke: {refusal}"
    );

    // THE UNMUTATED CONTROL: the same 4096 bytes with nothing to escape
    // render inside the cap and are accepted.
    let plain = Job {
        args: vec!["a".repeat(4096)],
        ..job("ordinary", NOW)
    };
    assert_eq!(validate_registration(&plain, NOW), Ok(()));
}

/// A RECORD WHOSE `id` IS NOT ITS FILENAME IS REFUSED RATHER THAN ACTED ON.
///
/// The id is what a repeat republishes under and what a cancel removes, so
/// a file `a-job` whose record says `id=other-job` could re-arm itself on
/// top of an unrelated job's record and replace it.
#[test]
fn a_record_whose_id_is_not_its_filename_is_refused() {
    let scratch = Scratch::new("id-must-match-the-filename");
    let spool = scratch.spool();
    let lying = spool.join("a-job");
    std::fs::write(&lying, format!("{}\n", render(&job("other-job", NOW)))).expect("a record");

    let Peeked::Unusable(refusal) = peek(&lying, "a-job") else {
        panic!("a record naming another id must be refused");
    };
    assert!(
        refusal.contains("other-job") && refusal.contains("a-job"),
        "the refusal must name both ids: {refusal}"
    );

    // THE UNMUTATED CONTROL: the same record under its own name is a job.
    let honest = spool.join("other-job");
    std::fs::write(&honest, format!("{}\n", render(&job("other-job", NOW)))).expect("a record");
    assert!(matches!(peek(&honest, "other-job"), Peeked::Job(_)));
}

/// A SYMLINK STANDING WHERE THE MARKERS DIRECTORY SHOULD BE CANCELS
/// NOTHING.
///
/// A validated marker name cannot escape the state directory by itself, but
/// a link AT the directory carries the whole lookup somewhere this tool did
/// not choose, which is the general filesystem probe the name rule exists
/// to prevent. Refused reads as no marker, so the job runs: one extra card
/// rather than a cancellation somebody else's symlink decided.
#[test]
fn a_symlinked_markers_directory_cancels_nothing() {
    let scratch = Scratch::new("markers-dir-must-be-real");
    let elsewhere = scratch.root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("another directory");
    std::fs::write(elsewhere.join("answered"), "").expect("a marker over there");
    std::os::unix::fs::symlink(&elsewhere, marker_dir(&scratch.root)).expect("the symlink");

    let waiting = Job {
        unless_marker: Some("answered".to_string()),
        ..job("nag", NOW)
    };
    assert!(
        !marker_exists(&scratch.root, &waiting),
        "a marker reached through a symlinked directory must not cancel a job"
    );

    // THE UNMUTATED CONTROL: a real directory with the same marker in it
    // cancels the job, so the refusal above is the link and not the name.
    let honest = Scratch::new("markers-dir-real");
    let markers = marker_dir(&honest.root);
    std::fs::create_dir_all(&markers).expect("the markers directory");
    std::fs::write(markers.join("answered"), "").expect("the marker");
    assert!(marker_exists(&honest.root, &waiting));
}

/// THE DOCTOR COUNTS JOBS, SO IT COUNTS ONLY WHAT COULD BE ONE.
///
/// The loop refuses to open an irregular entry and will never run it, so
/// counting it reports a job that cannot exist, in the one sentence an
/// operator reads to find out whether anything is scheduled.
#[test]
fn the_job_count_counts_records_and_not_whatever_is_in_the_directory() {
    let scratch = Scratch::new("job-count-is-jobs");
    let spool = scratch.spool();
    publish_job(&spool, &job("real", NOW)).expect("a real job");
    std::fs::create_dir_all(spool.join("a-directory")).expect("a directory in the spool");
    assert_eq!(
        job_count(&scratch.root),
        1,
        "only the record is a job; a directory is not one"
    );
}

/// A SPOOL PATH THAT IS NOT A DIRECTORY IS A PERMANENT REFUSAL.
///
/// `create_dir_all` follows a symlink, so the check has to come first, and
/// NOTHING A RETRY DOES CHANGES IT: that is why the caller exits 0 and lets
/// launchd keep the job down instead of relaunching it every ten seconds
/// forever.
#[test]
fn a_spool_path_that_is_not_a_directory_is_a_permanent_refusal() {
    let scratch = Scratch::new("spool-must-be-a-directory");
    std::fs::write(spool_dir(&scratch.root), "not a directory").expect("a file in the way");

    let Startup::Refused(refusal) = prepare_spool(&scratch.root) else {
        panic!("a file where the spool should be must refuse the start");
    };
    assert!(
        refusal.contains("is not a directory"),
        "the refusal must say what it found: {refusal}"
    );

    // THE UNMUTATED CONTROL: an absent spool is MADE rather than refused.
    let clean = Scratch::new("spool-is-made");
    assert_eq!(prepare_spool(&clean.root), Startup::Ready);
    assert!(spool_dir(&clean.root).is_dir(), "the spool is created");
}
