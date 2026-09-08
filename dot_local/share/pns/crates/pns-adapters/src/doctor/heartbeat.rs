/// Whether the clock is running, said in one line that grades nothing.
///
/// TWO READS THAT COST NOTHING: the heartbeat file, and a count of the spool.
/// IT DOES NOT SIGNAL THE PID, because a pid can be reused and the age of a
/// file the daemon rewrites every second answers the same question honestly.
/// `enabled` COMES FROM THE ONE CONFIG READ the doctor already took, never a
/// second one: a report assembled from two reads of one file can describe a
/// switch the run itself never saw. Its broken-config fallback is ON, the same
/// one `daemon_run` takes, so the report and the service cannot disagree.
pub fn daemon_heartbeat(state: &std::path::Path) -> Option<pns_domain::jobs::Heartbeat> {
    let path = crate::job_spool::heartbeat_path(state);
    // A NON-REGULAR FILE IS NOT A BEAT AND IS NEVER OPENED, the same refusal
    // the spool takes and for a worse reason: `open` on a FIFO blocks until a
    // writer arrives, so a doctor that read whatever it found there would hang
    // instead of printing any of its four states, with the pairing check and
    // the exit code never reached.
    matches!(std::fs::symlink_metadata(&path), Ok(found) if found.is_file())
        .then(|| std::fs::read_to_string(&path).ok())
        .flatten()
        .and_then(|line| pns_domain::jobs::parse_heartbeat(&line))
}
