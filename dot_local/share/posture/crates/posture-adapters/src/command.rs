use posture_application::InspectionFailure;
use std::ffi::OsStr;
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

pub trait CommandRunner {
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        merge_stderr: bool,
    ) -> Result<Vec<u8>, InspectionFailure>;
}

pub struct SystemRunner {
    expires: Instant,
}
impl SystemRunner {
    pub fn new(budget: Duration) -> Self {
        Self {
            expires: Instant::now() + budget,
        }
    }
}

struct OwnedChild(Option<Child>);
impl OwnedChild {
    fn exited(&self) -> Result<bool, InspectionFailure> {
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
    fn finish(&mut self) -> Result<ExitStatus, InspectionFailure> {
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

impl CommandRunner for SystemRunner {
    fn run(
        &mut self,
        program: &Path,
        args: &[&OsStr],
        merge_stderr: bool,
    ) -> Result<Vec<u8>, InspectionFailure> {
        if Instant::now() >= self.expires {
            return Err(InspectionFailure::TimedOut);
        }
        let (mut reader, writer) = io::pipe().map_err(|_| InspectionFailure::Unavailable)?;
        // The read descriptor is owned here; changing its flags cannot affect the writer.
        let flags = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_GETFL) };
        if flags == -1
            || unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) }
                == -1
        {
            return Err(InspectionFailure::Unavailable);
        }
        let stderr = if merge_stderr {
            Stdio::from(
                writer
                    .try_clone()
                    .map_err(|_| InspectionFailure::Unavailable)?,
            )
        } else {
            Stdio::null()
        };
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::from(writer))
            .stderr(stderr)
            .process_group(0);
        let mut child = OwnedChild(Some(
            command
                .spawn()
                .map_err(|_| InspectionFailure::Unavailable)?,
        ));
        // Command retains parent copies of the writers until dropped; EOF depends on this.
        drop(command);
        let mut output = Vec::new();
        let mut eof = false;
        loop {
            if Instant::now() >= self.expires {
                return Err(InspectionFailure::TimedOut);
            }
            let mut bytes = [0_u8; 4096];
            if !eof {
                match reader.read(&mut bytes) {
                    Ok(0) => eof = true,
                    Ok(count) => output.extend_from_slice(&bytes[..count]),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                    Err(_) => return Err(InspectionFailure::Failed),
                }
            }
            if eof && child.exited()? {
                let status = child.finish()?;
                return if status.success() {
                    Ok(output)
                } else {
                    Err(InspectionFailure::Failed)
                };
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

#[cfg(test)]
mod tests;
