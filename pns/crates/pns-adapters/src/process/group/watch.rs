use std::io;
use std::time::Instant;

pub(super) fn deadline(expires_at: Instant) -> io::Result<i64> {
    let remaining = expires_at.saturating_duration_since(Instant::now());
    let now = monotonic();
    if now < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(now.saturating_add(remaining.as_nanos().min(i64::MAX as u128) as i64))
}

// All routines below also run after fork in a potentially multithreaded parent.
// They use only stack values and native calls, never allocation, locks, logging,
// unwinding, or Rust destructors. Darwin's proc_pidinfo is a thin __proc_info
// syscall wrapper (xnu/libsyscall/wrappers/libproc/libproc.c); unlike readdir it
// has no userspace allocator or lock. The other libc calls are async-signal-safe.
fn monotonic() -> i64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: clock_gettime writes the valid stack timespec.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return -1;
    }
    time.tv_sec
        .saturating_mul(1_000_000_000)
        .saturating_add(time.tv_nsec)
}

/// The command joins this child's group only after its readiness byte.
/// A deliberately detached descendant that creates another group is outside it.
pub(super) unsafe fn run(owner: i32, ready: i32, until: i64, group: i32) -> ! {
    // SAFETY: all descriptors and process operations are local to this fork child.
    unsafe {
        if libc::setpgid(0, group) != 0 {
            libc::_exit(1);
        }
        if !close_inherited(owner, ready, until) {
            libc::_exit(1);
        }
        if libc::write(ready, [1u8].as_ptr().cast(), 1) != 1 {
            libc::_exit(1);
        }
        libc::close(ready);
        let mut pipe = libc::pollfd {
            fd: owner,
            events: libc::POLLIN,
            revents: 0,
        };
        loop {
            let now = monotonic();
            if now < 0 || now >= until {
                break;
            }
            let timeout = ((until - now) as u64)
                .div_ceil(1_000_000)
                .min(i32::MAX as u64) as i32;
            let result = libc::poll(&mut pipe, 1, timeout);
            if result > 0 {
                break;
            }
            // A signal may interrupt poll. Recompute the original deadline.
            // Other poll failures are refused by terminating the owned group.
            if result < 0 && *libc::__error() != libc::EINTR {
                break;
            }
        }
        // The guardian stays in this group until termination, so the group
        // cannot disappear and be confused with a recycled unrelated group.
        libc::kill(-libc::getpgrp(), libc::SIGKILL);
        libc::_exit(1);
    }
}

// Inspect this fork child's stable table, not a racy snapshot from its parent.
// Repeated bounded batches cover sparse high descriptors and every file type.
// No descriptor ceiling determines the amount of work before command launch.
unsafe fn close_inherited(owner: i32, ready: i32, until: i64) -> bool {
    let mut entries = [libc::proc_fdinfo {
        proc_fd: 0,
        proc_fdtype: 0,
    }; 128];
    loop {
        let now = monotonic();
        if now < 0 || now >= until {
            return false;
        }
        // SAFETY: the syscall writes at most this valid stack buffer's bytes.
        let bytes = unsafe {
            libc::proc_pidinfo(
                libc::getpid(),
                libc::PROC_PIDLISTFDS,
                0,
                entries.as_mut_ptr().cast(),
                std::mem::size_of_val(&entries) as i32,
            )
        };
        let record_size = std::mem::size_of::<libc::proc_fdinfo>();
        if bytes <= 0
            || bytes as usize > std::mem::size_of_val(&entries)
            || !(bytes as usize).is_multiple_of(record_size)
        {
            return false;
        }
        let count = bytes as usize / record_size;
        let mut kept = 0;
        for entry in entries.iter().take(count) {
            let fd = entry.proc_fd;
            if fd < 0 {
                return false;
            }
            if fd == owner {
                kept |= 1;
            } else if fd == ready {
                kept |= 2;
            } else if unsafe { libc::close(fd) } != 0 {
                // A failed close cannot establish safe readiness.
                return false;
            }
        }
        if count < entries.len() {
            return kept == 3;
        }
        // A full batch has at least 126 unrelated descriptors. They are now
        // closed; a subsequent query must progress toward the two retained ends.
    }
}
