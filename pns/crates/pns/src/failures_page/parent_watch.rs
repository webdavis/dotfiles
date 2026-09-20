//! The page's tie to the daemon that started it.
//!
//! THE PAGE IS A DETACHED CHILD IN A GROUP OF ITS OWN, so a daemon that stops
//! leaves it holding the port with nothing left to reap it. A page from a
//! four-day-old binary then outlives every restart, and each new daemon's child
//! can never bind.
//!
//! KQUEUE IS THE WAKE-UP AND `getppid` IS THE TRUTH. `EVFILT_PROC` with
//! `NOTE_EXIT` reports the moment the watched process leaves, and the ppid read
//! on that wake is what decides: a pid the kernel has recycled, or a
//! registration this could not place, both end in a reparented child the next
//! read catches.
//!
//! A FIRED WATCH IS SPENT, so this polls from there on: the one-shot
//! registration is gone once it has been delivered, and a second wait on the
//! same queue would park for the whole recheck no matter what the ppid says.

use std::time::Duration;

/// How long a placed watch waits before reading the ppid anyway.
const RECHECK_AFTER: Duration = Duration::from_secs(30);

/// How often the ppid is read when no watch could be placed, which is the only
/// case with nothing to wake this up.
const POLL_AFTER: Duration = Duration::from_secs(1);

/// Exit this process once the process that started it is gone.
///
/// A THREAD RATHER THAN A CHECK IN THE ACCEPT LOOP, because the page spends its
/// life blocked in `accept` and a reader who never comes must not be what keeps
/// an orphan alive.
pub(crate) fn exit_when_orphaned() {
    let parent = parent_pid();
    std::thread::spawn(move || {
        let mut watch = Watch::on(parent);
        loop {
            if orphaned(parent) {
                std::process::exit(0);
            }
            watch.wait();
        }
    });
}

/// Whether the process that started this one is gone.
///
/// A CHANGED PPID OR THE REAPER ITSELF, because a child can be orphaned before
/// it ever reads its own parent: the daemon that spawned it may be stopped
/// while this is still starting up, and a page that took pid 1 for its parent
/// would then watch the one process that never exits.
fn orphaned(parent: libc::pid_t) -> bool {
    let now = parent_pid();
    now != parent || now <= 1
}

/// SAFETY: `getppid` takes no arguments, touches no memory this process owns
/// and cannot fail.
fn parent_pid() -> libc::pid_t {
    unsafe { libc::getppid() }
}

/// A kqueue watching one process, or nothing if one could not be placed.
enum Watch {
    Queue(Queue),
    Poll,
}

impl Watch {
    fn on(pid: libc::pid_t) -> Self {
        match Queue::on(pid) {
            Some(queue) => Self::Queue(queue),
            None => Self::Poll,
        }
    }

    /// Block until the watched process exits or the recheck falls due, giving
    /// up the queue once it has fired.
    fn wait(&mut self) {
        match self {
            Self::Queue(queue) => {
                if queue.wait(RECHECK_AFTER) {
                    *self = Self::Poll;
                }
            }
            Self::Poll => std::thread::sleep(POLL_AFTER),
        }
    }
}

/// An owned kqueue descriptor with one `NOTE_EXIT` registration on it.
struct Queue(std::os::fd::OwnedFd);

impl Queue {
    fn on(pid: libc::pid_t) -> Option<Self> {
        use std::os::fd::FromRawFd;
        // SAFETY: `kqueue` takes no arguments and returns either a descriptor
        // this becomes the sole owner of or -1.
        let raw = unsafe { libc::kqueue() };
        if raw < 0 {
            return None;
        }
        // SAFETY: `raw` is a fresh descriptor no other object owns.
        let queue = Self(unsafe { std::os::fd::OwnedFd::from_raw_fd(raw) });
        queue.register(pid).then_some(queue)
    }

    fn register(&self, pid: libc::pid_t) -> bool {
        let change = event(pid);
        // SAFETY: the descriptor is live, `change` is one initialised event
        // this call only reads, and no result events are asked for.
        let placed = unsafe {
            libc::kevent(
                self.raw(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        placed >= 0
    }

    /// Whether the registration fired, rather than the timeout falling due.
    fn wait(&self, timeout: Duration) -> bool {
        let deadline = libc::timespec {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_nsec: 0,
        };
        let mut received = event(0);
        // SAFETY: the descriptor is live, no changes are submitted, and the
        // kernel writes at most the one event `received` has room for.
        let events =
            unsafe { libc::kevent(self.raw(), std::ptr::null(), 0, &mut received, 1, &deadline) };
        events != 0
    }

    fn raw(&self) -> libc::c_int {
        std::os::fd::AsRawFd::as_raw_fd(&self.0)
    }
}

/// One `EVFILT_PROC` registration for `pid`, fired once and then forgotten: a
/// process exits once, and the ppid read on that wake is what acts on it.
fn event(pid: libc::pid_t) -> libc::kevent {
    libc::kevent {
        ident: pid as libc::uintptr_t,
        filter: libc::EVFILT_PROC,
        flags: libc::EV_ADD | libc::EV_ONESHOT,
        fflags: libc::NOTE_EXIT,
        data: 0,
        udata: std::ptr::null_mut(),
    }
}
