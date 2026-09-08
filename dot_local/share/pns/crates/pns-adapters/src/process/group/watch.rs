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
// They use only stack values and async-signal-safe libc calls, never allocation,
// locks, logging, unwinding, or Rust destructors.
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
pub(super) unsafe fn run(owner: i32, ready: i32, max_fd: i32, until: i64, group: i32) -> ! {
    // SAFETY: all descriptors and process operations are local to this fork child.
    unsafe {
        if libc::setpgid(0, group) != 0 {
            libc::_exit(1);
        }
        for fd in 0..max_fd {
            if fd != owner && fd != ready {
                libc::close(fd);
            }
            if fd % 64 == 0 {
                let now = monotonic();
                if now < 0 || now >= until {
                    libc::_exit(1);
                }
            }
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
