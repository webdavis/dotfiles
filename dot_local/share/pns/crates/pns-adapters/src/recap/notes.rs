use pns_application::{Fetched, ReviewNoteSource};
use std::{
    io::Read,
    os::unix::fs::OpenOptionsExt,
    path::Path,
    time::{Duration, UNIX_EPOCH},
};

pub struct ReviewNotes {
    pub home: String,
}
impl ReviewNoteSource for ReviewNotes {
    fn notes(&self, pattern: &str, since: u64, until: u64) -> Option<Fetched> {
        notes_matching(pattern, &self.home, since, until)
    }
}

/// The review notes the glob names whose own clock falls inside the window, or
/// None when the directory the operator named could not be read at all.
///
/// THE GLOB IS THE WHOLE PERMISSION and this is where that is spent: one
/// directory, named in full by the operator, listed once. Nothing recurses,
/// nothing follows a name the pattern did not match, and the config layer has
/// already refused a pattern whose DIRECTORY carries a `*`, so the set of
/// directories pns opens is a statement rather than a search.
///
/// THE WINDOW IS `activity_in`'S OWN PREDICATE, so a note is in this recap for
/// the same reason an event is: it happened after the operator was last here.
/// A note they had already read when they left is not news.
///
/// EVERY READ IS BOUNDED and every file is a file: a directory or a device
/// entry matching the pattern is skipped rather than opened, and what is read
/// stops at a ceiling, because this is an ordinary directory other tools also
/// write into.
///
/// NEWEST FIRST, WHICH IS WHAT THE CAP THEN CUTS. Sorting by name and taking
/// the first `MAX_NOTES` kept whatever sorted earliest, so `checklist-a*.md`
/// outranked the note written an hour ago, which is the opposite of what a
/// section about the night wants. The name breaks a tie, so one window still
/// renders the same way twice.
///
/// ACCEPTED LIMIT: past `MAX_NOTES` the count is a FLOOR rather than a total,
/// which is the honesty `header` states about a pruned ring. The MESSAGE says
/// so now: `Fetched::truncated` is what turns the section's remainder into "at
/// least", so a glob matching forty notes cannot print a count that reads as a
/// total.
fn notes_matching(pattern: &str, home: &str, since: u64, until: u64) -> Option<Fetched> {
    let expanded = match pattern.strip_prefix("~/") {
        Some(rest) => Path::new(home).join(rest),
        None => std::path::PathBuf::from(pattern),
    };
    let name = expanded.file_name()?.to_str()?.to_string();
    let mut matched: Vec<(Duration, std::path::PathBuf)> = std::fs::read_dir(expanded.parent()?)
        .ok()?
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|found| matches_glob(found, &name))
        })
        .filter_map(|path| Some((modified_at(&std::fs::metadata(&path).ok()?)?, path)))
        .filter(|(at, _)| within(*at, since, until))
        .collect();
    matched.sort_by(|(left, left_path), (right, right_path)| {
        right.cmp(left).then_with(|| left_path.cmp(right_path))
    });
    Some(Fetched {
        truncated: matched.len() > MAX_NOTES,
        sources: matched
            .iter()
            .take(MAX_NOTES)
            .filter_map(|(_, path)| {
                let named = path.file_name()?.to_str()?;
                // A NOTE THAT WOULD NOT OPEN IS STILL A NOTE. It matched the
                // operator's own pattern and its clock puts it in the window,
                // so dropping it renders a night in which that finding never
                // existed; the mode, the race or the swap that stopped the read
                // is exactly what they would want to see.
                Some(match read_note(path, since, until) {
                    Some(contents) => pns_domain::recap::external::noted(named, &contents),
                    None => pns_domain::recap::external::unreadable(named),
                })
            })
            .collect(),
    })
}

/// Whether one name matches a pattern holding at most one `*`, which is the
/// only glob the config layer admits. Everything else is a literal, so a
/// pattern names one file.
fn matches_glob(name: &str, pattern: &str) -> bool {
    match pattern.split_once('*') {
        None => name == pattern,
        Some((head, tail)) => {
            name.len() >= head.len() + tail.len() && name.starts_with(head) && name.ends_with(tail)
        }
    }
}

/// One file's own clock, or None when it has none this can read.
fn modified_at(metadata: &std::fs::Metadata) -> Option<Duration> {
    metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()
}

/// Whether a clock puts a file inside the window, on `activity_in`'s half-open
/// rule and AT FULL PRECISION. Truncating to whole seconds excluded a file
/// written half a second after the marker and admitted one written half a
/// second after the window closed, which is the one edge each rule exists to
/// place.
fn within(at: Duration, since: u64, until: u64) -> bool {
    at > Duration::from_secs(since) && at <= Duration::from_secs(until)
}

/// One matched note read up to a ceiling, through a handle that is CHECKED
/// AFTER IT IS OPEN.
///
/// OPEN THEN VERIFY, because the scan and the read are two moments and a
/// directory other tools write into can change between them. The open refuses
/// to follow a link at all (`O_NOFOLLOW`), so a symlink dropped at a name the
/// glob matched cannot widen the read past the one directory the pattern names;
/// and the file type and the clock are re-read off the HANDLE, so a file
/// rewritten after the scan cannot feed this window contents from outside it.
/// Checking the path a second time instead would be the same race with more
/// steps: the answer would still describe whatever the name pointed at then.
///
/// LOSSY, for `run_bounded`'s reason: this is a plain file other tools write,
/// and one invalid byte must cost its own character rather than the note.
fn read_note(path: &Path, since: u64, until: u64) -> Option<String> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || !within(modified_at(&metadata)?, since, until) {
        return None;
    }
    let mut text = Vec::new();
    Read::take(&mut file, NOTE_READ_MAX)
        .read_to_end(&mut text)
        .ok()?;
    Some(String::from_utf8_lossy(&text).into_owned())
}

/// How many review notes one recap considers, and how much of each it reads.
const MAX_NOTES: usize = 25;

const NOTE_READ_MAX: u64 = 64 * 1024;

#[cfg(test)]
#[path = "notes/tests.rs"]
mod recap_sources_runtime_tests;
