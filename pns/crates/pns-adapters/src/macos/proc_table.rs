//! The phone chain's process walk, taken out of `libproc` in process.
//!
//! `libc` already declares everything but the `PROC_ALL_PIDS` selector and
//! `devname`, so this costs no dependency at all.

use std::ffi::{CStr, c_char, c_int};

/// The one question the phone chain asks the process table: which terminals
/// belong to the children of these processes.
///
/// ONE STEP RATHER THAN TWO, because the record that names a process's parent
/// carries its controlling terminal device as well, so the `ps -o tty=` spawn
/// the chain used to make has nothing left to ask.
pub trait ProcessTable: Send + Sync + 'static {
    /// The controlling terminal names of every child of `parents`. A child
    /// with no terminal, or one the kernel would not name, is left out.
    fn child_terminals(&self, parents: &[i32]) -> Vec<String>;
}

/// The live process table.
pub struct LibprocTable;

impl ProcessTable for LibprocTable {
    fn child_terminals(&self, parents: &[i32]) -> Vec<String> {
        if parents.is_empty() {
            return Vec::new();
        }
        all_pids()
            .into_iter()
            .filter_map(bsd_info)
            .filter(|info| parents.contains(&(info.pbi_ppid as i32)))
            .filter_map(|info| terminal_name(info.e_tdev))
            .collect()
    }
}

/// Every process id on the machine, or an empty list where the table could
/// not be read.
///
/// SIZED, THEN FILLED, with room to spare: the table can grow between the two
/// calls, and the kernel fills only as much as it was given, so the spare
/// entries are what keeps a process that started in that window from pushing
/// the one being looked for out of the answer.
fn all_pids() -> Vec<i32> {
    // SAFETY: a null buffer of size zero asks libproc for the byte count
    // only, which is the documented sizing call.
    let sized = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
    if sized <= 0 {
        return Vec::new();
    }
    let capacity = sized as usize / PID_BYTES + SPARE_PIDS;
    let mut pids = vec![0i32; capacity];
    let bytes = match c_int::try_from(capacity * PID_BYTES) {
        Ok(bytes) => bytes,
        Err(_) => return Vec::new(),
    };
    // SAFETY: the buffer holds exactly `bytes` bytes, which is what libproc
    // is told it may write.
    let written = unsafe { libc::proc_listpids(PROC_ALL_PIDS, 0, pids.as_mut_ptr().cast(), bytes) };
    if written <= 0 {
        return Vec::new();
    }
    pids.truncate((written as usize / PID_BYTES).min(capacity));
    pids.retain(|pid| *pid > 0);
    pids
}

/// One process's BSD record, or None where it has gone or refused the read.
fn bsd_info(pid: i32) -> Option<libc::proc_bsdinfo> {
    let mut info = std::mem::MaybeUninit::<libc::proc_bsdinfo>::zeroed();
    let size = c_int::try_from(size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: the buffer is one whole `proc_bsdinfo` and its size is what
    // libproc is told, so a short write cannot run past it.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            size,
        )
    };
    // A SHORT ANSWER IS NO ANSWER: libproc reports how much it filled, and a
    // partial record would carry an uninitialized parent or device.
    // SAFETY: the record was fully written, which is what the check above
    // establishes.
    (written == size).then(|| unsafe { info.assume_init() })
}

/// The `/dev` name of a terminal device number, refused unless it is a plain
/// name.
fn terminal_name(device: u32) -> Option<String> {
    if device == NO_DEVICE {
        return None;
    }
    // SAFETY: `devname` answers into a static buffer it owns, which is read
    // and copied before returning; only the one probe thread calls it.
    let name = unsafe {
        let named = devname(device as libc::dev_t, libc::S_IFCHR);
        if named.is_null() {
            return None;
        }
        CStr::from_ptr(named).to_string_lossy().into_owned()
    };
    plain_device_name(&name).map(str::to_string)
}

/// THE TRUST BOUNDARY, because the name is about to become a path under
/// `/dev`: anything not plain alphanumeric is refused outright, so no reading
/// can carry a slash or a `..` into that join.
pub(crate) fn plain_device_name(name: &str) -> Option<&str> {
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric())).then_some(name)
}

/// `PROC_ALL_PIDS`, which `libc` does not declare.
const PROC_ALL_PIDS: u32 = 1;
/// `NODEV`, the device number of a process with no controlling terminal.
const NO_DEVICE: u32 = u32::MAX;
const PID_BYTES: usize = size_of::<i32>();
/// Room for processes that start between the sizing call and the filling one.
const SPARE_PIDS: usize = 64;

unsafe extern "C" {
    fn devname(device: libc::dev_t, mode: libc::mode_t) -> *mut c_char;
}
