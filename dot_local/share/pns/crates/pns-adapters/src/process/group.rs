use std::io::{self, PipeWriter, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::time::Instant;

mod watch;

/// A live cleanup child pins the owned group until this owner reaps it.
/// Its pipe survives a producer crash, unlike a thread in the producer.
pub(crate) struct Group {
    owner: Option<PipeWriter>,
    pid: libc::pid_t,
    scope: Scope,
}

#[derive(Clone, Copy)]
enum Scope {
    Command,
    Recap(libc::pid_t),
}

impl Group {
    pub(super) fn start(expires_at: Instant) -> io::Result<Self> {
        Self::start_in(expires_at, Scope::Command)
    }

    pub(crate) fn for_recap(expires_at: Instant) -> io::Result<Self> {
        if expires_at <= Instant::now() {
            return Err(io::Error::other("recap deadline has already elapsed"));
        }
        // A hand-run recap can inherit a caller's group. Own a group before
        // arming termination, never signal the caller's or the operator's group.
        // SAFETY: these calls inspect or change only this process's group.
        let pid = unsafe { libc::getpid() };
        if unsafe { libc::getpgrp() } != pid && unsafe { libc::setpgid(0, 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Self::start_in(expires_at, Scope::Recap(pid))
    }

    fn start_in(expires_at: Instant, scope: Scope) -> io::Result<Self> {
        let (owner_read, owner_write) = io::pipe()?;
        let (mut ready_read, ready_write) = io::pipe()?;
        let until = watch::deadline(expires_at)?;
        // SAFETY: sysconf reads the process's descriptor limit, with no pointers.
        let max_fd = unsafe { libc::sysconf(libc::_SC_OPEN_MAX) };
        if max_fd < 0 || max_fd > i64::from(libc::c_int::MAX) {
            return Err(io::Error::other("cannot bound cleanup descriptors"));
        }
        // SAFETY: the child enters only async-signal-safe operations in watch::run
        // and ends with _exit or SIGKILL, never Rust allocation or destruction.
        let pid = unsafe { libc::fork() };
        if pid < 0 {
            return Err(io::Error::last_os_error());
        }
        if pid == 0 {
            // SAFETY: these descriptors belong to the copied child table. The
            // routine closes every other descriptor before announcing readiness.
            unsafe {
                watch::run(
                    owner_read.as_raw_fd(),
                    ready_write.as_raw_fd(),
                    max_fd as i32,
                    until,
                    match scope {
                        Scope::Command => 0,
                        Scope::Recap(pid) => pid,
                    },
                )
            }
        }
        let group = Self {
            owner: Some(owner_write),
            pid,
            scope,
        };
        drop(owner_read);
        drop(ready_write);
        let remaining = expires_at.saturating_duration_since(Instant::now());
        let mut ready = libc::pollfd {
            fd: ready_read.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll receives one valid descriptor record and a bounded timeout.
        let answered = unsafe {
            libc::poll(
                &mut ready,
                1,
                remaining.as_millis().min(i32::MAX as u128) as i32,
            )
        };
        let mut byte = [0];
        if answered <= 0 || ready_read.read_exact(&mut byte).is_err() || byte != [1] {
            return Err(io::Error::other("process cleanup did not become ready"));
        }
        Ok(group)
    }

    pub(super) fn spawn(&self, command: &mut Command) -> io::Result<Child> {
        // The standard library joins the existing group before exec. A missing
        // group refuses the launch, so no executable side effect precedes ownership.
        command.process_group(self.pid).spawn()
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        // SAFETY: pid is our unreaped fork child (>1), so neither its id nor its
        // anchored group can be reused. Kill also handles failed/stopped setup.
        unsafe {
            if matches!(self.scope, Scope::Command) {
                libc::kill(-self.pid, libc::SIGKILL);
            }
            libc::kill(self.pid, libc::SIGKILL);
            while libc::waitpid(self.pid, std::ptr::null_mut(), 0) < 0 {
                if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    break;
                }
            }
        }
        // A completed recap is a member of the watched group. Stop/reap its
        // guardian before closing the owner pipe, or EOF could kill success.
        drop(self.owner.take());
    }
}

#[cfg(test)]
mod tests;
