use super::*;
/// Every claim an earlier run left in the state directory, oldest first, plus
/// every hold whose owner did not live to give it up.
///
/// MATCHED ON THE JOURNAL'S OWN CLAIM PREFIX and nothing looser: the turn
/// marker claims itself in this directory too, under its own name, and a
/// wider match would hand a turn's start time to the replayer. The one
/// addition is an ABANDONED HOLD, which is a stranded batch in every way that
/// matters here and is admitted only once the run that took it is gone.
///
/// SORTED BY WHEN THEY WERE LAST WRITTEN, which is the journal's own
/// timestamp: a rename does not touch it, so a claim still carries the moment
/// its last entry was appended. A time that cannot be read sorts oldest, which
/// costs an ordering and never a delivery.
pub(super) fn stranded_claims(state: &Path) -> Vec<std::path::PathBuf> {
    let prefix = format!("{MISSED_NOTIFICATIONS}.claim.");
    let Ok(entries) = std::fs::read_dir(state) else {
        return Vec::new();
    };
    let mut found: Vec<(Option<SystemTime>, std::path::PathBuf)> = entries
        .flatten()
        .filter(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            name.starts_with(&prefix) || abandoned_hold(&name)
        })
        // `DirEntry::metadata` does not traverse a symlink, matching the
        // append's and the reader's own refusal to judge one by its target.
        .map(|entry| {
            (
                entry
                    .metadata()
                    .ok()
                    .and_then(|found| found.modified().ok()),
                entry.path(),
            )
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, path)| path).collect()
}
/// Whether a name is a HELD file whose owner is gone.
///
/// A held file is a batch some run had taken and was reading when it died, in
/// a window one rename wide. Nothing else may touch one while its owner lives,
/// which is the whole reason the name sits outside the claim prefix: an owner
/// that is still reading cannot have its batch taken a second time.
pub(super) fn abandoned_hold(name: &str) -> bool {
    name.strip_prefix(&format!("{MISSED_NOTIFICATIONS}.held."))
        .is_some_and(owner_is_gone)
}
