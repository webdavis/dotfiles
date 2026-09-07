use super::*;

#[test]
fn the_presence_line_is_read_once_however_often_it_is_asked_for() {
    // ONE PROBE SET IS ONE READING. The doctor asks and the routing will
    // ask again inside the same event; a second read could straddle the
    // daemon's next write and answer a different room from the first.
    let path = scratch_path("presence-once");
    std::fs::write(&path, b"1000 990 1 3F - Studio").unwrap();
    let probes = probes_at(&path);
    let first = probes.presence_line();
    std::fs::write(&path, b"2000 1990 1 2F - Kitchen").unwrap();
    let second = probes.presence_line();
    std::fs::remove_file(&path).ok();
    assert_eq!(first.as_deref(), Some("1000 990 1 3F - Studio"));
    assert_eq!(second, first, "the second ask must not re-read the file");
}

#[test]
fn the_presence_line_is_taken_when_the_path_is_pointed_and_never_after_a_clock() {
    // THE LINE AND THE CLOCK ARE COMPARED, so which is read first decides
    // whether a reading can come out NEWER than the moment it is judged
    // against. Taken lazily, the line was read whenever something first
    // asked, which is after `now_secs` has frozen on every path there is:
    // the event path asks once its plan is decided, the blocked path
    // freezes the clock before it forwards to moshi, and the doctor takes
    // the clock a statement before the line. The daemon republishes every
    // few seconds, so a line landing inside any of those windows read as
    // `Future` and gave the whole house back.
    let path = scratch_path("presence-before-the-clock");
    std::fs::write(&path, b"1000 990 1 3F - Studio").unwrap();
    let probes = probes_at(&path);
    // The daemon's next poll, published before anything asks for a line.
    std::fs::write(&path, b"2000 1990 1 2F - Kitchen").unwrap();
    let read = probes.presence_line();
    std::fs::remove_file(&path).ok();
    assert_eq!(
        read.as_deref(),
        Some("1000 990 1 3F - Studio"),
        "the reading is the one that was there when the set was pointed at it"
    );
}

#[test]
fn an_absent_presence_file_is_no_reading() {
    assert_eq!(
        probes_at(std::path::Path::new("/nonexistent/pns-presence")).presence_line(),
        None
    );
}

#[test]
fn a_fifo_at_the_presence_path_is_refused_on_its_metadata_and_never_opened() {
    // MEASURED ELSEWHERE IN THIS CRATE: opening a FIFO blocks until the
    // other end opens, for reading as much as for writing, so a read that
    // trusted the path would park the hook for the life of the machine.
    // The deadline is what makes a regression a FAILURE rather than a
    // suite that hangs with nothing to read.
    let path = scratch_path("presence-fifo");
    assert!(
        std::process::Command::new("/usr/bin/mkfifo")
            .arg(&path)
            .status()
            .expect("mkfifo runs")
            .success(),
        "the fixture has to be a real FIFO"
    );
    // THE READER RUNS DETACHED, never joined: a regression parks it on the
    // open for good, and a scoped thread would wait for it and hang the
    // suite instead of failing it.
    let (report, reading) = std::sync::mpsc::channel();
    let reader = path.clone();
    std::thread::spawn(move || report.send(probes_at(&reader).presence_line()));
    let answered = reading.recv_timeout(std::time::Duration::from_secs(5));
    std::fs::remove_file(&path).ok();
    assert_eq!(
        answered,
        Ok(None),
        "the FIFO was opened and the read parked"
    );
}

#[test]
fn a_presence_file_past_the_read_cap_is_no_reading() {
    let path = scratch_path("presence-huge");
    std::fs::write(&path, vec![b'x'; (crate::PRESENCE_READ_MAX + 1) as usize]).unwrap();
    let reading = probes_at(&path).presence_line();
    std::fs::remove_file(&path).ok();
    assert_eq!(reading, None);
}

#[test]
fn a_symlink_at_the_presence_path_is_no_reading() {
    // The LINK is judged, never its target: the state file is this tool's
    // own, and a link standing in for it is something another hand put
    // there.
    let target = scratch_path("presence-target");
    let link = scratch_path("presence-link");
    std::fs::write(&target, b"1000 990 1 3F - Studio").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let reading = probes_at(&link).presence_line();
    std::fs::remove_file(&link).ok();
    std::fs::remove_file(&target).ok();
    assert_eq!(reading, None);
}
