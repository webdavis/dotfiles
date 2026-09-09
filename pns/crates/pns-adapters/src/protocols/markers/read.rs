use std::path::Path;
/// One epoch off a state file, or None for anything this will not vouch for:
/// nothing at the path, a file that cannot be read, or text that is not a
/// plain count.
///
/// AN UNPARSEABLE MARKER IS NO EDGE AT ALL, never an edge at epoch zero. A
/// marker some other hand rewrote is not a near edge this can trust, and
/// reading one as zero would recap the whole ring.
pub fn read_epoch(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}
