use posture_application::InspectionFailure;
use std::io;
use std::process::{Child, ExitStatus};

pub(super) struct OwnedChild(pub(super) Option<Child>);
impl OwnedChild {
    pub(super) fn exited(&self) -> Result<bool, InspectionFailure> {
        let Some(child) = self.0.as_ref() else {
            return Err(InspectionFailure::Failed);
        };
        // A zeroed siginfo_t is valid input; waitid fills it when this owned child exits.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // WNOWAIT retains the child's pid until group termination, preventing pid reuse.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == -1 {
            return if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                Ok(false)
            } else {
                Err(InspectionFailure::Failed)
            };
        }
        // waitid succeeded and initialized the pid member (zero means no exited child).
        Ok(unsafe { info.si_pid() } != 0)
    }
    pub(super) fn finish(&mut self) -> Result<ExitStatus, InspectionFailure> {
        let Some(mut child) = self.0.take() else {
            return Err(InspectionFailure::Failed);
        };
        terminate_group(&child);
        child.wait().map_err(|_| InspectionFailure::Failed)
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            terminate_group(&child);
            let _ = child.wait();
        }
    }
}
fn terminate_group(child: &Child) {
    // The unreaped child still owns this pid and the process group created at spawn.
    unsafe {
        libc::kill(-(child.id() as i32), libc::SIGKILL);
    }
}
