//! The process table read in process, through `libproc`.
//!
//! `libc` 0.2.189 declares every call here but the `PROC_ALL_PIDS` selector,
//! so this costs no dependency.

use posture_application::InspectionFailure;
use std::ffi::c_int;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// One window for a walk, the same generous bound the spawned reads had: the
/// table answers in about a millisecond, so this is three orders of magnitude
/// of headroom and still a bound.
const WALK_DEADLINE: Duration = Duration::from_secs(5);

/// The process table, asked which process ids a name, a real user, a parent
/// and an executable directory select together.
pub trait ProcessLookup {
    /// The matching ids in ascending order, empty where the table holds no
    /// match. `uid`, `parent` and `directory` each go unfiltered when None.
    ///
    /// `directory` selects on where the executable lives: a process matches
    /// when its executable path sits at or below that directory.
    ///
    /// The name is the executable's own file name. macOS `pgrep -x` compares
    /// the name a process gave its own argument zero, which is the same string
    /// for every process posture asks about (measured 2026-09-20 on live
    /// osqueryd, OverSight and LuLu extension processes), and a different one
    /// for a process that renames itself.
    fn matching(
        &mut self,
        name: &str,
        uid: Option<u32>,
        parent: Option<u32>,
        directory: Option<&Path>,
    ) -> Result<Vec<i32>, InspectionFailure>;
}

/// The live process table.
pub struct LibprocProcesses {
    deadline: Duration,
}

impl Default for LibprocProcesses {
    fn default() -> Self {
        Self::with_deadline(WALK_DEADLINE)
    }
}

impl LibprocProcesses {
    pub fn with_deadline(deadline: Duration) -> Self {
        Self { deadline }
    }
}

impl ProcessLookup for LibprocProcesses {
    fn matching(
        &mut self,
        name: &str,
        uid: Option<u32>,
        parent: Option<u32>,
        directory: Option<&Path>,
    ) -> Result<Vec<i32>, InspectionFailure> {
        let name = name.to_owned();
        let directory = directory.map(Path::to_path_buf);
        bounded_call(self.deadline, move || {
            walk(&name, uid, parent, directory.as_deref())
        })
        .ok_or(InspectionFailure::TimedOut)?
        .ok_or(InspectionFailure::Failed)
    }
}

/// Runs the read on a thread of its own so a wedged table read is left
/// behind rather than waited on; no answer is the caller's unknown.
pub(crate) fn bounded_call<T: Send + 'static>(
    deadline: Duration,
    call: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .spawn(move || {
            let _ = sender.send(call());
        })
        .ok()?;
    receiver.recv_timeout(deadline).ok()
}

/// The selected ids in ascending order, or None where the table itself could
/// not be read.
fn walk(
    name: &str,
    uid: Option<u32>,
    parent: Option<u32>,
    directory: Option<&Path>,
) -> Option<Vec<i32>> {
    let mut selected: Vec<i32> = all_pids()?
        .into_iter()
        .filter(|pid| short_info(*pid).is_some_and(|info| owned_by(&info, uid, parent)))
        .filter(|pid| executable_path(*pid).is_some_and(|path| runs(&path, name, directory)))
        .collect();
    // ASCENDING, so two walks of one unchanged table answer identically; the
    // kernel's own listing order is not promised anywhere.
    selected.sort_unstable();
    Some(selected)
}

/// Whether one executable path carries the name being looked for and lives
/// under the directory being looked in.
fn runs(path: &Path, name: &str, directory: Option<&Path>) -> bool {
    path.file_name().and_then(|file| file.to_str()) == Some(name)
        && directory.is_none_or(|directory| path.starts_with(directory))
}

/// Whether one record passes the real-user and parent filters. The REAL user
/// id is the one compared, which is what `pgrep -U` matches.
fn owned_by(info: &libc::proc_bsdshortinfo, uid: Option<u32>, parent: Option<u32>) -> bool {
    uid.is_none_or(|uid| info.pbsi_ruid == uid) && parent.is_none_or(|ppid| info.pbsi_ppid == ppid)
}

/// Every process id on the machine, or None where the table could not be read.
///
/// SIZED, THEN FILLED, with room to spare: the table can grow between the two
/// calls and the kernel fills only as much as it was given, so the spare
/// entries are what keeps a process that started in that window from pushing
/// the one being looked for out of the answer.
fn all_pids() -> Option<Vec<i32>> {
    // SAFETY: a null buffer of size zero asks libproc for the byte count
    // only, which is the documented sizing call.
    let sized = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if sized <= 0 {
        return None;
    }
    let capacity = sized as usize / PID_BYTES + SPARE_PIDS;
    let mut pids = vec![0i32; capacity];
    let bytes = c_int::try_from(capacity * PID_BYTES).ok()?;
    // SAFETY: the buffer holds exactly `bytes` bytes, which is what libproc
    // is told it may write.
    let written = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), bytes) };
    if written <= 0 {
        return None;
    }
    pids.truncate((written as usize / PID_BYTES).min(capacity));
    pids.retain(|pid| *pid > 0);
    Some(pids)
}

/// One process's short BSD record, or None where it has gone.
///
/// The SHORT record is the one every process answers: the full
/// `PROC_PIDTBSDINFO` record is refused with EPERM for a process owned by
/// another user (measured 2026-09-20: launchd, osqueryd and the LuLu
/// extension all refuse it from an ordinary account), and the parent, the
/// real user and the name are all the walk asks for.
fn short_info(pid: i32) -> Option<libc::proc_bsdshortinfo> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdshortinfo>::zeroed();
    let size = c_int::try_from(size_of::<libc::proc_bsdshortinfo>()).ok()?;
    // SAFETY: the buffer is one whole `proc_bsdshortinfo` and its size is what
    // libproc is told, so a short write cannot run past it.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDT_SHORTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    // A SHORT ANSWER IS NO ANSWER: libproc reports how much it filled, and a
    // partial record would carry an uninitialized parent or user.
    // SAFETY: the record was fully written, which is what the check above
    // establishes.
    (written == size).then(|| unsafe { info.assume_init() })
}

/// The path of a process's executable, or None where it could not be read.
///
/// The PATH rather than the record's own name field, because that field holds
/// at most thirty one bytes and the LuLu extension's name is thirty two.
fn executable_path(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let size = u32::try_from(buffer.len()).ok()?;
    // SAFETY: the buffer holds `size` bytes, which is the ceiling libproc is
    // given, and the call writes a NUL-terminated path no longer than that.
    let written = unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), size) };
    if written <= 0 {
        return None;
    }
    buffer.truncate(written as usize);
    Some(PathBuf::from(String::from_utf8(buffer).ok()?))
}

/// `PROC_ALL_PIDS`, which `libc` does not declare.
const PROC_ALL_PIDS: u32 = 1;
const PID_BYTES: usize = size_of::<i32>();
/// Room for processes that start between the sizing call and the filling one.
const SPARE_PIDS: usize = 64;

#[cfg(test)]
mod tests;
