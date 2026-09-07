use posture_application::InspectionFailure;
use std::io;

pub(super) struct Foreground {
    original: libc::pid_t,
    child: libc::pid_t,
    active: bool,
}
impl Foreground {
    pub(super) fn take(child: libc::pid_t) -> Result<Option<Self>, InspectionFailure> {
        // Descriptor 0 is inherited input. A pipe or a non-controlling terminal needs no handoff.
        let original = unsafe { libc::tcgetpgrp(0) };
        if original == -1 {
            return match io::Error::last_os_error().raw_os_error() {
                Some(libc::ENOTTY | libc::EBADF) => Ok(None),
                _ => Err(InspectionFailure::Failed),
            };
        }
        // Never take the terminal from an unrelated foreground job.
        if original != unsafe { libc::getpgrp() } {
            return Ok(None);
        }
        set_foreground(child)?;
        // A child can reach read before the handoff and stop on SIGTTIN. It is still owned/unreaped.
        unsafe {
            libc::kill(-child, libc::SIGCONT);
        }
        Ok(Some(Self {
            original,
            child,
            active: true,
        }))
    }
    pub(super) fn restore(&mut self) -> Result<(), InspectionFailure> {
        if self.active {
            self.active = false;
            // A shell may have reclaimed its terminal after a signal; do not steal it back.
            if unsafe { libc::tcgetpgrp(0) } == self.child {
                set_foreground(self.original)?;
            }
        }
        Ok(())
    }
}
impl Drop for Foreground {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}
fn set_foreground(group: libc::pid_t) -> Result<(), InspectionFailure> {
    // Block SIGTTOU only in this thread while changing foreground ownership. Restore its exact
    // previous mask afterward, including when tcsetpgrp fails. No process-wide handler is replaced.
    unsafe {
        let mut blocked: libc::sigset_t = std::mem::zeroed();
        let mut original: libc::sigset_t = std::mem::zeroed();
        if libc::sigemptyset(&mut blocked) == -1
            || libc::sigaddset(&mut blocked, libc::SIGTTOU) == -1
            || libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut original) != 0
        {
            return Err(InspectionFailure::Failed);
        }
        let changed = libc::tcsetpgrp(0, group);
        let restored = libc::pthread_sigmask(libc::SIG_SETMASK, &original, std::ptr::null_mut());
        if changed == -1 || restored != 0 {
            Err(InspectionFailure::Failed)
        } else {
            Ok(())
        }
    }
}
