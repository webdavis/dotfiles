use std::io::Write;
use std::path::Path;

/// Renew the lease this pane holds, if it holds one.
///
/// THE PANE'S ORDINARY HOOK TRAFFIC IS THE RENEWAL, which is what makes the
/// lease a liveness signal rather than a timer: an agent that is still working
/// is still firing events from its own pane, and one that stopped stops
/// renewing. Nothing else in this crate renews it.
///
/// IT CREATES NOTHING, and that is a property of the WRITE rather than of a
/// check in front of one. The open states no `create`, so the file has to be
/// there already, and the bytes go through the HANDLE rather than through a
/// fresh file renamed over the path: a `pns loop end` that lands after the open
/// sends these bytes to an inode nobody can reach any more, where a look-then-
/// publish would have written the lease back into existence and left the lamp
/// breathing for a whole timeout over work that had finished.
///
/// IT WRITES IN PLACE RATHER THAN TRUNCATING FIRST, so a tick reading the file
/// mid-renewal cannot see an empty one and sweep the lease. Both epochs are ten
/// digits and will be for the next two centuries, so a read caught between the
/// two sees a mix of two same-length numbers, which is a second or two out
/// rather than a lease nobody can parse. The `set_len` after the write is for
/// the day that stops being true.
pub fn renew_loop_lease(state: &Path, pane: &str, now: Option<u64>) {
    let (Some(marker), Some(now)) = (crate::marker_files::lease_marker(state, pane), now) else {
        return;
    };
    // The failures are DROPPED here: a lease that did not renew costs the lamp
    // one timeout, and this process has no reader for a complaint.
    let line = format!("{now}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(&marker)
        && file.write_all(line.as_bytes()).is_ok()
    {
        let _ = file.set_len(line.len() as u64);
    }
}
